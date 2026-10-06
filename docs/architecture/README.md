# Architecture

Status: milestones 0 to 4 are built and merged (§6); the rest is **proposed** · companion to `SPEC.md` · decisions logged as A-numbers in `decisions.md`

This is the structural architecture under the hood. `SPEC.md` and `docs/decisions.md` own product behaviour; this folder owns how the code is split up, how the parts talk, and in what order they get built. The design prototype's code is reference only and is deliberately not mirrored.

| File                    | Owns                                                                               |
| ----------------------- | ---------------------------------------------------------------------------------- |
| `README.md` (this file) | Layers, the composition model, data flow, windows, milestones, risks               |
| `crates-and-plugins.md` | The concern-by-concern split into crates and Tauri plugins, and which are reusable |
| `frontend.md`           | Front-end framework evaluation, the chosen stack and the front-end structure       |
| `ci-cd.md`              | The CI/CD pipeline, following the Liminal HQ house conventions                     |
| `remote-locations.md`   | The remote and virtual location contract: URIs, providers, credentials, transfers  |
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
- **Errors are typed.** One error enum per crate with stable codes; the UI maps codes to the designed states (permission denied with administrator access, not enough space, invalid or duplicate name, disconnected or unreachable remote, a login or host-key question, failed job with Retry). Remote providers' connection errors are listed in `remote-locations.md` §3.3 (A80).
- **Native → Rust signals stay in Rust** (portal, D-Bus and shell signals go through a plugin channel to Rust, never through the webview), per the Threshold Channels rule.

## 4. Windows

One frontend bundle; the entry point routes on the webview label. There is no static main window: no browser window is declared in `tauri.conf.json`, and the composition root creates each `main-{n}` from the restored session (a new session opens one window at Home), through a `WindowFactory` injected into the session plugin. Labels are never reused within a process (A40). Geometry is captured from `Moved` and `Resized` events (throttled to 250 ms; on Wayland size and maximised only) and fitted to a visible monitor on restore, so `tauri-plugin-window-state` is gone. Closing a window's last tab closes the window, and closing the last window quits after a final save of `session.json` in the app data directory (D91). A command that opens a window (a new window, or a hand-off to a new window) is refused with a typed error once 12 windows are open, and logs a warning from the eighth (the spike measured about 160 ms and 330 MB per window); a new window is cascaded 30 logical pixels down and right of the window that opened it on X11 and Windows, fitted to a visible monitor, and left to the compositor on Wayland, which ignores positions.

| Label             | Purpose                                                                |
| ----------------- | ---------------------------------------------------------------------- |
| `main-{n}`        | A browser window (tab strip, panes, inspector, terminal drawer, shelf) |
| `settings`        | Settings, plugin manager, developer options                            |
| `properties-{id}` | Floating Properties (Alt+Enter)                                        |
| `ops`             | Popped-out operations queue                                            |
| `shelf`           | The Shelf, undocked from the main windows (at most one)                |
| `tear-ghost`      | The pre-created, hidden, click-through ghost used during tab tear-off  |

The `shelf` window is not a window of the session store (it has no tabs, so it never counts toward the window cap or keeps the app alive) but a recipient of it: while the Shelf is undocked (`ShelfWindow` in the store, A73) it reads a snapshot, issues the Shelf commands and hears the Shelf's events, so the one writer stays the store. Capabilities are granted per label, least privilege. Tab, group, pair and layout state for every window lives in one `waypoint-session` store in Rust (A37), so tear-off, merge-by-drop and session restore are atomic backend operations (`MoveTabs`, A38) that hand a tab (location, history, split state, group, pin, colour) to the target window; only the scroll position and the focused entry travel as UI hints. The session is saved through `tauri-plugin-store` (A39).

**Tear-off and merge (milestone 3, slice 10).** `src-tauri` registers `tauri-plugin-window-tearoff` with `tear-ghost` as the ghost label; the plugin creates the ghost, not `tauri.conf.json`, and `capabilities/tear-off.json` grants it to `main-*` and `tear-ghost` only. Each window reads `get_status` once at start-up (`useTearoffFeatures`) and keeps the flags. `tabs/tearOff.ts` is the drag engine's `TearOffHook`: the in-page card always follows the pointer inside its window; with `ghost` and `cursor_follow` the plugin's ghost takes over outside it, and a timer (the page gets no pointer events outside its window) asks `hit_test` which registered region the cursor is over to change the ghost's label to "Release to merge into …". On release `end('drop')` reports the cursor and the hit: a region of another window becomes `moveTabs` to `ExistingWindow { label, index }`, no region a `NewWindow` whose `Geometry` puts the inner origin at `cursor − grab × scale` (nothing when the cursor is stale or `window_position` is off, so the window cascades or is placed by the compositor). Each window registers its strip (`strip`, merge at the end) and the two halves of every visible tab (`slot:N`, merge at index N) with `set_drop_regions`, only where `hit_test` works, throttled and cleared when it closes (`tabs/dropRegions.ts`). On a Wayland compositor without `xdg-toplevel-drag` all of this reduces to the in-page card and a compositor-placed window, and the menu paths do the merging (D94). Where the compositor has it, the plugin's `toplevel_drag` feature takes over (slice 13): the tabs move to a window the factory leaves hidden (`hold_next_window`), `begin_toplevel_drag` attaches that real window to a compositor drag through `crates/wayland-toplevel-drag`, and the page that holds the tabs when `window-tearoff://toplevel-drag-ended` arrives merges, returns or keeps them (`tabs/tearOffHandoff.ts`). The final executable exports the C interposer the crate needs, so `apps/waypoint/src-tauri/build.rs` sets two link arguments; `docs/tauri-tear-off.md` has the design and its pitfalls.

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

| #   | Milestone                                     | Delivers                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| --- | --------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 0   | **Risk spikes** (throwaway, in a scratch app) | See §7. Confirms or changes A2, A3, A9 and the tear-off approach; spikes 3 and 4 run with milestone 3.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| 1   | **Skeleton**                                  | `apps/waypoint` shell, `waypoint-protocol` + `ts-rs` drift check, `packages/chrome` (title bar, window menu, context menu), tokens, CI jobs for Rust/JS/drift/headers, `tauri:dev` with the MCP bridge                                                                                                                                                                                                                                                                                                                                                                                      |
| 2   | **Browse (local)**                            | `waypoint-path`, `waypoint-vfs` local provider + watcher, listing handles, virtualised list/grid, tab strip, sidebar, path bar, status bar                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| 3   | **Windows and tabs**                          | `waypoint-session` (one store for all windows), reusable `window-tearoff`, tear-off, merge, pairs, groups, pinned tabs, closed-tab history, session restore; `native-dnd` is spiked here and built in milestone 4                                                                                                                                                                                                                                                                                                                                                                           |
| 4   | **Operations**                                | `waypoint-ops`, reusable `trash` and `native-dnd`, `waypoint-settings` store, the undo journal, conflict resolver, verification, drag-and-drop engine and Shelf, batch rename, the Trash place with its browsable view; the Settings window (General, Operations, Drag & drop), the application menu, the action bar and the command palette with undo history                                                                                                                                                                                                                              |
| 5   | **System fit**                                | reusable `system-appearance`, `window-effects`, `thumbnails`, `volumes`, `mime-apps`; extend `xdg-portal` / `desktop-integration`; Services panel and the Settings → Integrations page; Settings → Appearance, Transparency, Accessibility (high contrast, text size, touch mode, reduced motion) and Language & Region (English and French, direction), icon style; Open With, the Inspector with thumbnails, Quick Look, sidebar Devices and the Overview place; the OS integration list (notifications, dock and taskbar progress, global shortcut, sleep inhibit, default file manager) |
| 6   | **Remotes and virtual locations**             | The remote contract (`remote-locations.md`, A78 to A85): remote, archive and Git URIs in `waypoint-path`, the extended `Provider` contract with a shared conformance suite, reusable `secrets`, `waypoint-connections` (saved connections held by the vfs plugin), provider crates (SFTP first, then SMB, WebDAV, S3, archives, Git), transfers across providers with resumable remote jobs, queue speed limits                                                                                                                                                                             |
| 7   | **Search, terminal, tags**                    | `waypoint-search`, reusable `pty`, tags/xattr; Inspector tag, comment and permission editing, terminal history, Flow integration and the terminal, Flow and Open With defaults on Settings → Integrations (the page itself arrives in milestone 5); an optional search index, switched on and off in settings, with its size and entry count shown there                                                                                                                                                                                                                                    |
| 8   | **Extensions**                                | `waypoint-ext` registry, manifest, permission model; bundled features (Find Duplicates, Compare and Sync, Previous Versions, Network Sharing, Nearby Devices, Disk Tools, Encrypted Vaults) re-expressed as first-party extensions; runtime decision (A7); Developer options; WebAssembly-only extensions that are sideloaded (no hosted index), and Nearby Devices over local discovery and the LocalSend protocol                                                                                                                                                                         |
| 9   | **Windows 11 parity, packaging, release**     | Windows modules for each plugin, MSI/portable/AppImage/deb/rpm, `release.yml`; first-run tour, F1 Help, About and update channels, crash reports and privacy settings, the full accessibility audit; Open as Administrator (polkit and UAC); Windows code signing, the sparse MSIX package and Windows 10 are out of scope for now, and crash reports are saved from Settings, never sent                                                                                                                                                                                                   |
| 10  | **Customisation and polish**                  | Keyboard shortcut editor and Nemo/Dolphin/Explorer presets, context-menu customisation, Window Layouts, the "Open startup tabs" setting and the Tabs & windows toggles, Sync Navigation, Vim mode, touchpad gestures, groups spanning windows (`open-questions.md`), multi-selection travelling with a hand-off, the remaining Settings pages (Keyboard, Tabs & windows); the Columns, Compact and Disk usage views, groups that span windows, and a FileChooser portal backend                                                                                                             |

Status: milestones 0 (the listing spike and the milestone 3 spikes), 1, 2, 3 and 4 are merged; milestone 5 is merged (verified in `milestone-5-verification.md`); milestone 6 is built on `main`, with a first Linux pass in `milestone-6-verification.md`; milestones 7 to 10 have not started.

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
