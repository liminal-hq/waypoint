# Architecture

Status: **proposed** (nothing here is built yet) · companion to `SPEC.md` · decisions logged as A-numbers in `decisions.md`

This is the structural architecture under the hood. `SPEC.md` and `docs/decisions.md` own product behaviour; this folder owns how the code is split up, how the parts talk, and in what order they get built. The design prototype's code is reference only and is deliberately not mirrored.

| File                    | Owns                                                                               |
| ----------------------- | ---------------------------------------------------------------------------------- |
| `README.md` (this file) | Layers, the composition model, data flow, windows, milestones, risks               |
| `crates-and-plugins.md` | The concern-by-concern split into crates and Tauri plugins, and which are reusable |
| `frontend.md`           | Front-end framework evaluation, the chosen stack and the front-end structure       |
| `ci-cd.md`              | The CI/CD pipeline, following the Liminal HQ house conventions                     |
| `decisions.md`          | Architecture decision log                                                          |

## 1. Where this comes from

Reviewed for house patterns: **Threshold** (Rust owns state, event hub, `ts-rs` generated types, plugin manifest and command conventions, `COMMANDS`/permissions rules), **Spindle** (pnpm workspace, plugins that own a domain, zustand, TanStack Router), **Cadence** (Bun workspace, shared M3-style component library, ported title bar, org-shared plugins consumed from `tauri-plugins-workspace`), **Jar** (the closest template: pure `crates/` core, `jar-protocol` with `ts-rs`, a thin `tauri-plugin-jar` adapter, Bun workspaces, the fullest CI/release pipeline), and **`tauri-plugins-workspace`** (the shared, published plugins: `xdg-portal`, `desktop-integration`, `material-you`, and the plugin template). Threshold and Spindle use pnpm; Cadence and Jar use Bun. Waypoint follows the newest choice, **Bun**.

## 2. The model in one picture

```
                    ┌─────────────────────────── apps/waypoint ───────────────────────────┐
                    │  src/  React 19 + TS: renders state, owns only ephemeral UI state   │
                    │        one bundle, routed by window label                          │
                    │  src-tauri/  composition root: registers plugins, binds traits      │
                    └───────┬───────────────────────┬────────────────────────┬────────────┘
        typed guest-js APIs │  Channels + events    │ generated ts-rs types  │
   ┌────────────────────────┴──────┐  ┌─────────────┴──────────────┐  ┌──────┴───────────────┐
   │ Domain plugins (in repo)      │  │ Reusable plugins           │  │ Shared (existing)    │
   │ waypoint-vfs · -ops ·         │  │ trash · thumbnails ·       │  │ xdg-portal ·         │
   │ -session · -search · -ext     │  │ volumes · secrets · pty ·  │  │ desktop-integration  │
   │ thin adapters                 │  │ window-tearoff · native-dnd│  │ (extended)           │
   └───────────────┬───────────────┘  │ window-effects ·           │  └──────────────────────┘
                   │                  │ system-appearance          │
   ┌───────────────┴───────────────┐  └────────────────────────────┘
   │ Pure crates (no tauri dep)    │       Reusable plugins hold their own logic and
   │ waypoint-protocol · -path ·   │       import nothing from Waypoint.
   │ -vfs · provider crates · -ops │
   │ -session · -search · -ext     │
   └───────────────────────────────┘
```

Three tiers, one direction of dependency:

1. **Pure crates** (`crates/*`) hold domain logic that is large, worth testing headlessly, or reusable outside Tauri. No `tauri` dependency.
2. **Plugins** (`plugins/*`) are the Tauri surface: commands, events, managed state, permissions, per-platform modules, and a `guest-js` package. Domain plugins are thin adapters over crates. Reusable plugins are self-contained and must not import any Waypoint crate.
3. **The app** (`apps/waypoint`) composes them. `src-tauri` registers plugins and binds the concrete implementations to the traits the crates define. It contains no concern logic.

**Plugins never call each other.** When one concern needs another (an operation needs the virtual file system, Trash and the keyring), the _crate that needs it defines a trait_ (`Trash`, `SecretStore`, `ProviderRegistry`) and the app crate injects the implementation, which is usually backed by a reusable plugin's Rust API. That keeps every plugin publishable and every crate testable with fakes.

## 3. Data flow

- **Rust owns state; one writer per piece of state.** Every mutation bumps a monotonic revision and emits granular events in a fixed order (the Threshold event-hub pattern). The frontend subscribes and renders; on reconnect or window open it asks for a snapshot at a revision.
- **Streams use `Channel`s.** Directory listings, job progress, search results, PTY output and watcher changes are pushed over Tauri `Channel`s. No polling, and no single giant `invoke` returning a whole directory.
- **Listings are handles, not arrays.** `list(location, query)` returns a listing id; Rust holds the sorted/filtered index and the frontend requests windows of rows (`get_range(listing, start, end)`) for the virtualised list, receiving `changed` events from the watcher. This keeps a 500 000-entry folder out of the JS heap. Milestone 0 confirms or replaces this (A9).
- **Types are generated.** Wire types are authored once in `crates/waypoint-protocol` and generated to TypeScript with `ts-rs`. CI regenerates and diffs (see `ci-cd.md`).
- **Errors are typed.** One error enum per crate with stable codes; the UI maps codes to the designed states (permission denied with administrator access, not enough space, invalid or duplicate name, disconnected remote, failed job with Retry).
- **Native → Rust signals stay in Rust** (portal, D-Bus and shell signals go through a plugin channel to Rust, never through the webview), per the Threshold Channels rule.

## 4. Windows

One frontend bundle; the entry point routes on the webview label. There is no static main window: no browser window is declared in `tauri.conf.json`, and the composition root creates each `main-{n}` from the restored session (a new session opens one window at Home), through a `WindowFactory` injected into the session plugin. Labels are never reused within a process (A40).

| Label             | Purpose                                                                |
| ----------------- | ---------------------------------------------------------------------- |
| `main-{n}`        | A browser window (tab strip, panes, inspector, terminal drawer, shelf) |
| `settings`        | Settings, plugin manager, developer options                            |
| `properties-{id}` | Floating Properties (Alt+Enter)                                        |
| `ops`             | Popped-out operations queue                                            |
| `tear-ghost`      | The pre-created, hidden, click-through ghost used during tab tear-off  |

Capabilities are granted per label, least privilege. Tab, group, pair and layout state for every window lives in one `waypoint-session` store in Rust (A37), so tear-off, merge-by-drop and session restore are atomic backend operations (`MoveTabs`, A38) that hand a tab (location, history, split state, group, pin, colour) to the target window; only the scroll position and the focused entry travel as UI hints. The session is saved through `tauri-plugin-store` (A39).

## 5. Cross-cutting rules

- **Availability everywhere.** Every plugin exposes a status command. The app aggregates them into the Services panel and hides unavailable options (`docs/os-integrations.md`, principle 3).
- **Platform modules.** `linux.rs`, `windows.rs`, `unsupported.rs` behind a single trait per concern; no scattered `#[cfg]` inside logic. macOS is not a target; `unsupported` returns "unavailable".
- **Portals first, Flatpak-safe.** Anything with an XDG portal uses it; direct access is the fallback (D45).
- **Secrets never touch Waypoint's files.** Only the `secrets` plugin talks to Secret Service or Credential Manager (D44).
- **Extensions are sandboxed, native plugins are trusted.** The extension host (`waypoint-ext`) enforces declared permissions; native plugins are compiled in and audited like any other code.
- **Accessibility and localisation are structural** (roles, focus, message catalogue), not a later pass (`docs/accessibility.md`, D80).
- **Observability.** `tauri-plugin-log` on the Rust and JS sides; a developer-options toggle logs portal and D-Bus calls (D47).

## 6. Milestones

Each milestone scaffolds only what it needs.

| #   | Milestone                                     | Delivers                                                                                                                                                                                                          |
| --- | --------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 0   | **Risk spikes** (throwaway, in a scratch app) | See §7. Confirms or changes A2, A3, A9 and the tear-off approach; spikes 3 and 4 run with milestone 3.                                                                                                            |
| 1   | **Skeleton**                                  | `apps/waypoint` shell, `waypoint-protocol` + `ts-rs` drift check, `packages/chrome` (title bar, window menu, context menu), tokens, CI jobs for Rust/JS/drift/headers, `tauri:dev` with the MCP bridge            |
| 2   | **Browse (local)**                            | `waypoint-path`, `waypoint-vfs` local provider + watcher, listing handles, virtualised list/grid, tab strip, sidebar, path bar, status bar                                                                        |
| 3   | **Windows and tabs**                          | `waypoint-session` (one store for all windows), reusable `window-tearoff`, tear-off, merge, pairs, groups, pinned tabs, closed-tab history, session restore; `native-dnd` is spiked here and built in milestone 4 |
| 4   | **Operations**                                | `waypoint-ops`, reusable `trash`, undo journal, conflict resolver, verification, drag-and-drop engine + Shelf                                                                                                     |
| 5   | **System fit**                                | reusable `system-appearance`, `window-effects`, `thumbnails`, `volumes`; extend `xdg-portal` / `desktop-integration`; Services panel                                                                              |
| 6   | **Remotes and virtual locations**             | `secrets`, provider crates (SFTP first, then SMB, WebDAV, S3, archives, Git)                                                                                                                                      |
| 7   | **Search, terminal, tags**                    | `waypoint-search`, reusable `pty`, tags/xattr                                                                                                                                                                     |
| 8   | **Extensions**                                | `waypoint-ext` registry, manifest, permission model; bundled features re-expressed as first-party extensions; runtime decision (A7)                                                                               |
| 9   | **Windows 11 parity, packaging, release**     | Windows modules for each plugin, MSI/portable/AppImage/deb/rpm, `release.yml`                                                                                                                                     |

Windows modules are written alongside each Linux module from milestone 2 onward, not deferred to milestone 9; milestone 9 is the parity audit and packaging.

## 7. Milestone 0 — risk spikes

Run alongside milestone 1 (A14), not before it. Throwaway experiments, each with a written result appended to `decisions.md`:

1. **Big-listing render (done; see [`milestone-0-spikes.md`](milestone-0-spikes.md)).** A virtualised list of 100 000–500 000 rows fed by a Rust listing handle with range fetch, measured for scroll and selection latency on **WebKitGTK (Wayland and X11)** and **WebView2**. Confirms React + TanStack Virtual, or triggers the Solid/Svelte fallback in `frontend.md`.
2. **Transparency and blur.** `transparent: true` plus `backdrop-filter` and compositor blur on GNOME/Mutter, KDE/KWin, Cinnamon and Windows 11 Mica.
3. **Tear-off (not yet run; scheduled with milestone 3, A35).** Ghost window follow, multi-monitor scale conversion, and the Wayland fallback path; cross-window hit-testing for merge. Also the cost of an extra WebKitGTK window (A36).
4. **Native drag and drop in and out (run in milestone 3, A36; Windows runtime behaviour untested).** Inbound file drops with positions, outbound drag to other apps and a terminal, and the `dragDropEnabled` interaction with an in-page pointer drag engine on Windows.
5. **Listing throughput (done on local files; see [`milestone-0-spikes.md`](milestone-0-spikes.md)).** `read_dir` + metadata streaming over a `Channel` on a 500 000-entry directory, local and over SFTP, with the watcher active.

## 8. Risks

| Risk                                                                              | Mitigation                                                                                                                                |
| --------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| WebKitGTK performance and rendering gaps                                          | Milestone 0 spikes 1 and 2; virtualise everything; keep the JS heap small via Rust-held listings                                          |
| Wayland restrictions (window positioning, always-on-top ghosts, global shortcuts) | Fallbacks designed into `window-tearoff` and `desktop-integration`; test on GNOME and KDE                                                 |
| Outbound native drag is not built into Tauri                                      | Dedicated `native-dnd` plugin, spiked in milestone 0                                                                                      |
| Scope: dozens of features across two operating systems                            | Concern-per-plugin split, milestones that each ship something usable, availability gating so partial platform support degrades gracefully |
| Extension runtime is an open question (`docs/open-questions.md` #3)               | Build the extension-point registry and permission model first with first-party extensions; pick the runtime in milestone 8                |
| Shared-plugin churn across repos                                                  | Incubate in-repo, graduate on a checklist (`crates-and-plugins.md` §4)                                                                    |
