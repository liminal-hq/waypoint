# AGENTS.md

## Table of Contents

- [Project Status](#project-status)
- [Localization and Spelling](#localization-and-spelling)
- [Markdown Formatting](#markdown-formatting)
- [Authoring Voice](#authoring-voice)
- [Commit Messages](#commit-messages)
- [Pull Request Titles](#pull-request-titles)
- [Pull Request Content](#pull-request-content)
- [Pull Request Labels](#pull-request-labels)
- [Git Workflow](#git-workflow)
- [Local Tooling](#local-tooling)
- [CI and Release](#ci-and-release)
- [Frontend Code Conventions](#frontend-code-conventions)
- [Architecture Rules](#architecture-rules)
- [Documentation](#documentation)
- [Repository Layout](#repository-layout)
- [Licence and Copyright](#licence-and-copyright)
- [Tauri v2](#tauri-v2)

## Project Status

Waypoint is a tabbed, extensible file manager for Linux (primary) and Windows 11 (a real target), built with Tauri v2, React/TypeScript and Rust. The repository is in **early development**: the skeleton (milestone 1), the listing spikes (milestone 0) **Milestone 2 (Browse, local)**, **Milestone 3 (Windows and tabs)** and **Milestone 4 (Operations)** are merged, and the reusable window plugins have graduated to the shared `tauri-plugins-workspace`. Milestone 3 delivered multiple windows, pinned and coloured tabs, groups, pairs, Recently Closed, `Ctrl+Tab` in most-recently-used order, workspaces, tear-off and merge, and session restore; deferred items are in #58 and the unverified platforms (hardware pointer, KDE, fractional scale, multi-monitor) in #54. `crates/waypoint-session` is the one pure store for every window's tabs, groups, pairs, pins, colours, history, closed tabs and workspaces; `plugins/waypoint-session` holds it behind one lock, sends each window granular events and saves it under the app's `ca.liminalhq.waypoint` data directory, and the page renders it through `TabsApi`. Tear-off and merge come from the reusable `plugins/window-tearoff` (and, on Wayland, `crates/wayland-toplevel-drag`), driven by the tab drag engine in `apps/waypoint/src/tabs`. Milestone 4 (Operations) made Waypoint write to disk: create, rename, duplicate, trash and restore, delete, and copy and move through a job queue with conflict and error dialogs, optional verification and a persisted undo journal with crash recovery, plus batch rename, the file clipboard, in-page and native drag and drop (in and out), the Shelf, the Trash view, the Settings window, the app menu, the Action bar and the command palette; its verification pass is #104 and its deferred items are in #86. `crates/waypoint-ops` is the pure job engine (planner, queue, executors, copy and move, journal) and reaches the file system only through a `waypoint_vfs::Provider`; `plugins/waypoint-ops` runs it on a bounded worker pool, and the reusable `plugins/trash` and `plugins/native-dnd` supply the Trash and the OS drag and drop and clipboard. Scaffold only what the current milestone needs. Update this section (and drop the "planned" qualifiers in [Repository Layout](#repository-layout)) as each part is actually scaffolded.

The prototype that the product design came from lives in a Claude Design project and is **reference only**; see `docs/ui-mockups/README.md`. Only the _behaviour_ it demonstrates is authoritative, and only via `SPEC.md` and the docs. Do not port, adapt or structurally mirror its code.

## Localization and Spelling

**REQUIREMENT:** All UI strings, code variables, comments, commit messages, pull request descriptions, and documentation MUST use **Canadian English** spelling.

Examples:

- `colour` instead of `color`
- `centre` instead of `center`
- `neighbour` instead of `neighbor`
- `behaviour` instead of `behavior`
- `cancelled` instead of `canceled`
- `customise` instead of `customize`, `minimise` instead of `minimize`
- `licence` (noun) vs `license` (verb) — though in UI context usually "Licence"

Keep American spellings where an external API, CSS property, crate or protocol requires them (for example `color`, `background-color`, `color-scheme`, `Cancel` in a third-party enum).

## Markdown Formatting

**REQUIREMENT:** Do not hard-wrap markdown prose. Write each paragraph or bullet as a single unwrapped line in the source, no matter how long — let the renderer (GitHub, a browser, an editor's soft-wrap) reflow it for display. This applies to markdown-rendered content: PR descriptions, docs under `docs/`, README files, `SPEC.md`, `SCREENS.md`. Commit message bodies are the one exception — hard-wrap those (see [Commit Messages](#commit-messages)); `git log`/`git show` in a terminal don't reflow long lines the way GitHub's PR view does.

- Manual line breaks mid-paragraph don't survive Markdown rendering as intended, and they create noisy diffs when a later edit only changes one word but reflows the whole wrapped block.
- This does not apply to genuinely separate list items, headings, or intentional line breaks (e.g. two-space trailing breaks, blank lines between paragraphs) — only to breaking up one continuous sentence/paragraph across multiple lines.
- Code comments are not markdown-rendered and may wrap normally at a reasonable line length, same as any other source line — this rule doesn't reach them.

**Em dashes:** use a real em dash (`—`) in prose, never `--` as a substitute. Applies everywhere prose appears — PR/issue descriptions, docs, README files, `SPEC.md`, commit messages, and code comments — independent of the hard-wrap rule's narrower scope above. Doesn't apply to an actual double-hyphen that means something else in context (a CLI flag like `--check`, a numeric range, etc.).

## Authoring Voice

**REQUIREMENT:** Ship the result, not how the conversation arrived at it. Write every outward-facing line — code comments, identifier names, docs, PR descriptions — as the author of the artifact, for the reader who will encounter it later, not as a record of the debugging or review process that produced it.

- Don't reference "this PR", "the review", a reviewer's name, or a commit SHA inside code comments or PR prose. State the fact or the reasoning directly, as if it had always been true.
- When a comment gets edited more than once across a change, rewrite it as one clean explanation — don't leave layered fragments from each edit stacked on top of each other.
- Commit messages are the exception: they're a legitimate place to record _why_ a change happened, including review feedback or debugging context — that's what git history is for.

## Commit Messages

**Format:** Use Conventional Commits format (e.g., `feat: ...`, `fix: ...`, `docs: ...`, `test: ...`).

- Use `test:` for test-related changes, including fixes to tests themselves (do not use `fix:` unless it fixes application code).
- Use `ci:` for workflow changes, `build:` for dependency and toolchain changes, `chore:` for housekeeping.

**Body Requirements:**

- Explain what and why (not how)
- Use markdown: **bold**, _italics_, `code`, bullet lists
- **Backtick every code-level reference** — component/function/class/variable names, file and directory paths, CSS selectors/properties/values, npm and crate package names, route paths, HTML tag names, config keys, and CLI flags (e.g. `TabStrip`, `plugins/tauri-plugin-waypoint-vfs`, `:hover`, `overflow-x: hidden`, `@tanstack/react-router`, `<button>`, `--flag`). This applies inline in prose, not just in fenced code blocks. Plain-English descriptions and user-facing UI strings (button labels, screen names, dialog copy) use quotes instead, not backticks — they aren't code.
- **NO markdown headings** — use **bold labels** for sections (not always required)
- Hard-wrap paragraphs (~72-100 chars), unlike other markdown in this repo — see [Markdown Formatting](#markdown-formatting)

**Specific Updates:** Each commit message should reflect the specific changes made in that commit. Do not just recap the entire project history or scope. Focus on the now.

**Shell Interpolation Safety:**

- Do not pass markdown-heavy commit bodies directly via `git commit -m "..."` when they include backticks, `$()`, or shell-sensitive characters.
- Prefer writing the message to a file with a single-quoted heredoc and commit with `git commit -F <file>` to prevent shell expansion.
- After committing, verify the stored message with `git log -1 --pretty=fuller` and amend immediately if interpolation altered content.

## Pull Request Titles

**REQUIREMENT:** PR titles MUST be human-readable summaries of the PR change.

- Start with a capital letter, write in the imperative mood, and keep to roughly one 70-character line.
- Do not use Conventional Commit prefixes in PR titles (for example, no `feat:`, `fix:`, `chore:`).
- Describe the outcome or behaviour change, not internal process language.
- Keep title style consistent across every open PR in the same stack. If one title in a stack is updated, update the rest to match.
- Do not rename merged PRs unless explicitly requested.

## Pull Request Content

**Requirement:** PR titles and descriptions must not mention internal workflow artefacts.

- Do not mention deferred-review documents, internal queue labels, or internal-only planning notes in outward PR content.
- Use user-facing, outcome-focused language in PR titles and descriptions.
- Open pull requests ready for review by default. Only create a draft PR when the user explicitly asks for a draft or when there is a clearly communicated blocker that makes draft status necessary.

**PR Description Format:**

- Prefer a compact markdown structure with `## Summary` and `## Test plan`.
- Under `## Summary`, use `###` sub-sections when they help group the change cleanly (for example `### User-facing changes`, `### Plugins`, `### Documentation`).
- Under each summary section, use flat bullets with bold lead-ins for scanability.
- Keep the summary focused on outcomes and behaviour changes, not commit history or implementation chronology.
- Under `## Test plan`, use checklist bullets (`- [x]` / `- [ ]`) and include the concrete commands, validations, or remaining gaps.
- If something could not be verified, state that plainly at the end of `## Test plan` or immediately below it.

## Pull Request Labels

**Requirement:** Add labels to every PR when it is created or updated.

- Add at least one primary category label to every PR: `enhancement`, `bug`, `documentation`, `testing`, `ci`, `build`, or `chore`.
- Add shared operational labels where they help clarify handling: `infrastructure`, `internal`, `release`, `blocked`, `epic`, or `skip-changelog`.
- Add scope labels where helpful. Technology: `frontend`, `backend`, `rust`, `plugin`, `data-model`. Platform: `linux`, `windows`, `wayland`. Product area: `tabs`, `drag-and-drop`, `remote`, `extensions`, `operations`, `search`, `os-integration`, `theming`, `accessibility`, `localisation`, `design-system`, `performance`, `security`. Structure and process: `architecture`, `spike` (a Milestone 0 risk spike), `experimental`, `developer-experience`.
- Prefer the broader Liminal HQ label style over Conventional Commit terms. Use `enhancement` and `bug`, not `feat` or `fix`.
- Use `skip-changelog` only when a change should be excluded from generated release notes (the categories are defined in `.github/release.yml`).
- Keep labels accurate as scope changes during review.

## Git Workflow

**Requirement:** Do not push changes (especially force pushes) to the repository unless explicitly requested by the user.

- **Default branch:** `main`. Work on a topic branch; do not commit directly to `main`.
- **Fix branch naming:** When creating a branch for a fix, use `fix/issue-<number>-<short-description>` (for example, `fix/issue-19-tab-tear-off-offset`).
- **GitHub tooling:** Prefer the `gh` CLI for repository, pull request, label, review, and GitHub Actions work.

## Local Tooling

- **JS runtime and package manager:** **Bun workspaces** (not pnpm/npm), with the Node version pinned in `.node-version` for tooling that needs it. Formatting is Prettier (`.prettierrc`: tabs, single quotes, 100 columns).
- **Rust:** the toolchain is pinned in `rust-toolchain.toml`. If `cargo` is not available on the host, run Rust/Tauri commands in the `ghcr.io/liminal-hq/tauri-dev-desktop:latest` container against the checked-out workspace.
- **Validation gate:** `bun run validate` is the single local gate that mirrors CI and must pass before opening or updating a PR. It runs the format check, the licence-header check, `tsc`, Vitest, the app build, `cargo fmt`, `cargo clippy -D warnings` and `cargo nextest`.
- **Editor settings:** `.editorconfig` is authoritative (tabs, LF, UTF-8).

### MCP Automation Bridge

Once the app exists, an agent driving the running app (screenshots, DOM snapshots, JS eval, simulated input) uses the `tauri-mcp-server` MCP tool, which talks to `tauri-plugin-mcp-bridge` — a debug-only plugin (`#[cfg(debug_assertions)]`) that never exists in release builds. Start the app with `bun run tauri:dev`, **not** plain `tauri dev`: `tauri:dev` merges `src-tauri/tauri.conf.dev.json`, which sets `"withGlobalTauri": true`, the global the bridge's JS-eval callback needs. Without it, `webview_execute_js`, screenshots, DOM snapshots and `webview_wait_for` silently time out while non-JS calls keep working. Waypoint is multi-window, so target windows by label. `tauri:dev` also sets `WEBKIT_DISABLE_DMABUF_RENDERER=1`: on Wayland with WebKitGTK the undocked Web Inspector renders black without it. It is a development-only workaround (the shipped app is unaffected); leave it in the script rather than exporting it in a shell profile, and re-test without it when WebKitGTK is upgraded.

### Logging

Native Rust and webview output share one log stream. `tauri-plugin-log` is configured in `src-tauri/src/lib.rs` (`Trace` in debug builds, `Info` in release; stdout plus the rotating `Waypoint.log`; chatty `tungstenite` crates held at `Warn`), and `src/services/logger.ts` redirects every webview `console.*` call into it, tagged with the call site and prefixed with the window label (for example `[main-1]`). Rules:

- Every window entry calls `initLogger(label)` once at startup; new window kinds must too.
- Use `log::{trace,debug,info,warn,error}!` in Rust and `console.*` in the front end. Do not add a second logging path, and never log secrets, credentials or full file contents.
- Each window's capabilities must include `log:default` (`capabilities/logging.json` grants it to every window), or forwarding fails silently.
- The log file is under the app's log directory (on Linux, `~/.local/share/ca.liminalhq.waypoint/logs/`).

## CI and Release

CI and release follow the Liminal HQ house pipeline (Jar and Cadence are the references). The full design, and which milestone activates each job, is in `docs/architecture/ci-cd.md`.

- **Workflows** live in `.github/workflows/`: `ci.yml` (quality gate on every PR and push to `main`), `publish-test-results.yml` (a `workflow_run` workflow that publishes JUnit check runs so Dependabot and fork PRs work), and `release.yml` (tag-driven cross-platform bundles).
- **Rust jobs run in the `ghcr.io/liminal-hq/tauri-ci-desktop:latest` container** with `Swatinem/rust-cache`; JS jobs use `oven-sh/setup-bun`. Installs use `--frozen-lockfile`.
- **Generated-code drift is a CI failure.** The `ts-rs` output under `crates/waypoint-protocol` consumers is regenerated from scratch in CI and diffed against the committed copy.
- **Licence headers** are enforced by `scripts/check-headers.sh`.
- **Releases** are tagged `vX.Y.Z` (or `vX.Y.Z-beta.N`), must be on `main`, and `scripts/check-release-versions.sh` requires every manifest version to be synchronised before bundles are built. Do not hand-bump a single manifest.
- **Dependabot** groups `tauri*` / `@tauri-apps/*` updates and labels them `build` (npm, cargo) or `ci` (Actions).
- Never push tags, trigger releases or dispatch workflows unless explicitly asked.

## Frontend Code Conventions

- **No barrel files.** Don't create an `index.ts`/`index.tsx` that only re-exports from sibling files. Import directly from the file that defines the thing (e.g. `import { TabStrip } from '../tabs/TabStrip'`). Barrels obscure the real dependency graph and slow down tree-shaking and IDE "go to definition."
- **React 19 + TypeScript (strict) + Vite.** Function components only. One folder per area directly under `src/` (`app`, `tabs`, `browse`, `nav`, `sidebar`, `status`, `services`, …); see `docs/architecture/frontend.md` for the stack and the reasoning.
- **The frontend renders; Rust decides.** Filesystem, operations, session and permission logic lives in Rust. Frontend code reacts to commands and events and never re-derives domain rules. Ephemeral UI state (hover, drag pointer state, focus) stays in the frontend.
- **Styling:** CSS Modules plus CSS custom properties for the semantic tokens in `docs/theming-and-platforms.md`. No inline-style styling (the prototype's approach): a value that must be measured in script, such as a menu's position, is passed to the stylesheet as a CSS custom property (`style={{ '--wp-menu-x': '12px' }}`) and consumed by a rule in the module, never as `left`, `top` or a colour on the element. **Colour literals live only in token files** (`apps/waypoint/src/theme/tokens.css` and the chrome's `packages/chrome/src/tokens.css` defaults); component CSS uses `var(--wp-*)` with no inline fallback, and every property a component reads needs a default in the chrome's `tokens.css`. `packages/chrome/src/tokens.test.ts` enforces all three.
- **Accessibility is a build requirement, not a polish pass:** roles, names, focus order and keyboard paths follow `docs/accessibility.md`. A component that is not keyboard-operable is not done.
- **All user-visible strings go through the message catalogue** (RTL and localisation are in scope, D80) — no string literals in JSX. The milestone-one catalogue is `apps/waypoint/src/i18n/messages.ts` (`t('area.thing')`), and the chrome's labels are supplied from it by `i18n/chromeLabels.ts`; add a key there, never a literal in a component. `i18n/messages.test.ts` fails if a screen holds literal `title`, `description` or `label` copy. Where a short word is ambiguous, write a `// translator: ...` comment above its key; the translator tooling (`bun run i18n:export`, `i18n:import`, `i18n:status`, `check:i18n`) copies it into the exported `translations/*.json`, and after an English change run `bun run i18n:export` (see `docs/translating.md`).
- **JavaScript reaches a native plugin only through its `guest-js` package.** Every plugin ships typed `guest-js` functions (and typed event subscriptions) that own the command and event names. Application and package code imports those functions and never calls `invoke('plugin:...|...')` or listens for a plugin's event by string. A plugin is not done until its `guest-js` covers every command and event the front end uses. `scripts/check-plugin-boundaries.sh` enforces the invoke half in CI.
- **Module names never differ only by case.** `TabMenus.tsx` beside `tabMenus.ts` is two files on Linux and one on Windows and macOS, where `tsc` fails while Cargo still embeds a stale `dist`. Give the pure model of a component a distinct name (`tabMenuModel.ts`). `scripts/check-case-collisions.sh` (`bun run check:case`) enforces it in CI.

## Architecture Rules

The rules below are the enforceable core of `docs/architecture/`. Change the architecture docs in the same PR as any change that alters them.

- **Concern-first split.** Each concern (trash, thumbnails, volumes, secrets, PTY, tear-off, native drag and drop, window effects, system appearance, virtual file system, operations, session, search, extensions) is its own crate or Tauri plugin. The app crate composes them; it does not implement concerns itself.
- **Pure crates for logic, thin plugins for the Tauri surface.** Logic that is large, reusable outside Tauri, or worth testing headlessly lives in a `crates/*` crate with **no `tauri` dependency**. A plugin is the adapter: commands, events, managed state, permissions.
- **Plugins never call each other.** Plugins do not depend on one another's Tauri APIs. Cross-concern behaviour (for example an operation that needs the virtual file system, Trash and secrets) is composed in the app crate through traits defined in the pure crates.
- **Generic plugins stay generic.** A plugin in the "reusable" tier (see `docs/architecture/crates-and-plugins.md`) must not import any Waypoint crate or mention Waypoint concepts, so it can graduate to `liminal-hq/tauri-plugins-workspace` unchanged. Prefer extending an existing shared plugin (`xdg-portal`, `desktop-integration`, `material-you`) over creating a new one.
- **Rust owns state.** One writer per piece of state, a monotonic revision on every mutation, and granular events after each change (the Threshold event-hub pattern). Streaming data (directory listings, job progress) uses Tauri `Channel`s, not polled commands or a giant single `invoke`.
- **Shared types are generated.** Wire types are defined once in Rust (`crates/waypoint-protocol`) and generated to TypeScript with `ts-rs`. Never hand-edit generated files; a CI drift check fails on any difference.
- **Every plugin reports availability.** Each plugin exposes a status command (`available`, `reason`, feature flags) so the UI can hide what does not work on the current system and populate Settings → Integrations → Services (principle 3 of `docs/os-integrations.md`).
- **Platform code is `cfg`-gated per platform module** (`linux.rs`, `windows.rs`, `unsupported.rs`), never scattered `#[cfg]` inside logic. Unsupported platforms return an explicit "unavailable" status rather than panicking.
- **Terminology.** _Native plugins_ are build-time Tauri plugins. _Extensions_ are the user-installable, sandboxed add-ons in `docs/plugins.md` (called "Plugins" in the UI, `ext` in code). Do not use "plugin" unqualified in code identifiers or PR titles when it could mean either.
- **Capabilities are least-privilege and per window label.** Every plugin command reachable from a webview needs a `permissions/default.toml` entry, and each window label gets only the capabilities it needs.

## Documentation

- **Updates:** when user-facing behaviour, screens, or platform requirements change, update `SPEC.md` (and `SCREENS.md` once it exists); when a design decision is made or changed, add or amend a `docs/decisions.md` entry (D-numbers) in the same change; when structure, crate or plugin boundaries, or data flow change, update `docs/architecture/` (A-numbers in `docs/architecture/decisions.md`). Keep `README.md` in sync.
- **Docs are maintained alongside the code, not after it.** A PR that changes behaviour without touching the doc that describes it is incomplete.
- **Open questions:** resolve an item in `docs/open-questions.md` by moving the answer into the relevant decision log and deleting the question.
- **No hard wrapping:** see [Markdown Formatting](#markdown-formatting) above.

## Repository Layout

Cargo workspace + **Bun workspaces**, matching Jar and Cadence. Built so far: `apps/waypoint`, `packages/chrome`, `packages/protocol`, the crates `waypoint-protocol`, `waypoint-path`, `waypoint-vfs`, `waypoint-session`, `waypoint-settings`, `waypoint-ops` and `wayland-toplevel-drag`, and the plugins `system-appearance`, `window-manager`, `window-tearoff`, `native-dnd`, `trash`, `waypoint-vfs`, `waypoint-session`, `waypoint-settings` and `waypoint-ops`; everything else below is **planned** until it is scaffolded. The app's identifier is `ca.liminalhq.waypoint`. The authoritative design is `docs/architecture/`.

- `apps/waypoint` — the Tauri app: React/TypeScript frontend in `src/`, and `src-tauri/` as a thin composition root that registers plugins and wires crates together.
- `packages/chrome` — shared React chrome (title bar, window menu, context menu, settings shell) with no Waypoint domain imports, structured so it can be extracted for the other Liminal HQ apps.
- `packages/protocol` — the TypeScript types `ts-rs` generates from the domain crates' wire types (types only, never hand-edited); the app and each domain plugin's `guest-js` import from it by package name.
- `crates/*` — pure Rust, no `tauri` dependency: `waypoint-protocol` (shared types and `ts-rs` generation), `waypoint-path`, `waypoint-vfs`, `waypoint-session`, `waypoint-settings`, `waypoint-ops` and `wayland-toplevel-drag` are built; `waypoint-search`, `waypoint-ext`, `waypoint-connections` and the per-protocol `waypoint-provider-*` crates are planned (milestone 6 contract: `docs/architecture/remote-locations.md`).
- `plugins/*` — Tauri plugins (see `plugins/README.md`; `system-appearance`, `window-manager`, `window-tearoff`, `native-dnd`, `trash`, `waypoint-vfs`, `waypoint-session`, `waypoint-settings` and `waypoint-ops` are built), each a Rust crate plus a `guest-js` package. Two tiers: domain plugins (`tauri-plugin-waypoint-*`) and reusable plugins (`tauri-plugin-{name}`) that graduate to the shared workspace.
- `docs/` — product design docs (`decisions.md`, `interactions.md`, …), `architecture/` (structure, plugins, frontend, CI/CD, ADRs) and `ui-mockups/` (prototype pointer, reference only).
- `scripts/` — repo tooling (`check-headers.sh`, later `check-release-versions.sh`).
- `.github/` — workflows, `dependabot.yml`, `release.yml` (changelog categories).

## Licence and Copyright

**REQUIREMENT:** All source code files (Rust, TypeScript, JavaScript, shell) MUST include a licence and copyright header as the first content in the file (after a shebang, for scripts).

**Header format:**

For Rust (`.rs`) files:

```
// Brief one-line summary of what this file does.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
```

For TypeScript/JavaScript (`.ts`, `.tsx`, `.js`) files:

```
// Brief one-line summary of what this file does.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
```

**Rules:**

- The first line is a concise summary of the file's purpose (one sentence, no period).
- Place the header before any `use`, `import`, or `mod` statement, followed by one blank line.
- Do not add headers to generated files, configuration files (`.toml`, `.json`, `.yml`), or documentation (`.md`).
- When visiting an existing file that lacks a header, add one as part of the current change.
- `scripts/check-headers.sh` enforces this in CI; its exemption list covers generated and tool-scaffolded files only.

The dual licence is provided as `LICENSE-MIT` and `LICENSE-APACHE` in the repository root.

## Tauri v2

Waypoint uses **Tauri v2** for desktop (Linux, Windows). These are the house patterns and pitfalls carried over from Threshold, Spindle, Cadence and Jar:

### Platform Detection

Use `@tauri-apps/plugin-os` for reliable platform detection across all targets.

- The `platform()` function is **synchronous** and determined at compile time.
- Returns: `'linux' | 'macos' | 'ios' | 'freebsd' | 'dragonfly' | 'netbsd' | 'openbsd' | 'solaris' | 'android' | 'windows'`.
- Use it for conditional UI rendering; prefer a plugin's availability status over platform checks when deciding whether a feature exists.

### Tauri APIs

- Prefer Tauri plugins over web APIs when available (e.g., `@tauri-apps/plugin-fs` over the browser File API — though Waypoint's own file access goes through `waypoint-vfs`, never the generic fs plugin).
- Most Tauri v2 APIs are async — use `async/await`.
- Check plugin documentation for platform-specific limitations.

### Common v1 Pitfalls (Agent Guide)

Because many online resources refer to Tauri v1, older patterns may inadvertently be suggested.

**Configuration (`tauri.conf.json`) differences:** `tauri` → `app` (top-level rename); `build.distDir` → `frontendDist`; `build.devPath` → `devUrl` (URLs only, not paths); `tauri.allowlist` → **removed**, replaced by the capabilities system; `tauri.bundle` → moved to top-level.

**JavaScript API changes:** `@tauri-apps/api` now only exports `core`, `path`, `event`, `window`; everything else moved to `@tauri-apps/plugin-*` (`fs`, `dialog`, `shell`, `os`, etc.).

**Permissions & capabilities (critical):** v1's `allowlist` is replaced by the **capabilities** ACL system. Capability files live in `apps/waypoint/src-tauri/capabilities/`. Installing a plugin is **not** enough — permissions must be explicitly granted per plugin. Every command in a plugin's `COMMANDS` list needs a matching entry in its `permissions/default.toml`, or a webview `invoke()` silently fails the ACL check even though the Rust command exists. Rust-side calls bypass the ACL, so check the ACL first when an invoke "does nothing". See Threshold's `docs/plugins/command-conventions.md` for the full `COMMANDS`-scope rule.

**Rust changes:** many `tauri::api` modules moved to separate plugins; use `std::fs` or `tauri_plugin_fs` instead of `tauri::api::file`; menu and tray APIs moved to separate crates.

### Waypoint-specific pitfalls

- **Multiple webviews, one bundle.** Each window is a separate webview with its own JS heap; keep the entry bundle small and route by window label. Never assume state is shared across windows — it goes through Rust.
- **Native file drop vs in-page drag and drop.** Tauri's native drag-drop handler and HTML5 drag events conflict on some platforms (notably Windows). Waypoint runs its own pointer-event drag engine and gets native in/out drag through a dedicated plugin; set `dragDropEnabled` deliberately per window, and re-test on every platform after touching it.
- **Linux webview is WebKitGTK.** Performance, `backdrop-filter`, transparency and Wayland behaviour differ from Chromium and from WebView2. Verify visual and performance work on WebKitGTK (GNOME and KDE, Wayland and X11), not only in a browser.
- **Window frame and shadows differ per platform.** The OS draws the shadow on Windows and macOS (`shadow: true`); on Linux the frame draws it in CSS inside a transparent margin. `data-platform` on the root element selects the tokens, and Windows drops the CSS radius so only its native rounding applies. See `docs/theming-and-platforms.md` (D89).
- **Logical vs physical pixels.** Cursor and window positions from Rust are physical; `screenX`/`screenY` are logical. Convert with the monitor scale factor (see `docs/tauri-tear-off.md`).
- **Background work and window visibility.** Webview timers are throttled when a window is hidden; long-running work (jobs, watchers, search) runs in Rust and pushes events.
