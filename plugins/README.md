# Plugins

Tauri plugins: the thin native layer between Waypoint's Rust crates and the front end. Each is a Rust crate plus a `guest-js` package, laid out to match [`tauri-plugins-workspace`](https://github.com/liminal-hq/tauri-plugins-workspace) so a plugin can graduate there unchanged. The split, the two tiers and the graduation checklist are in [`docs/architecture/crates-and-plugins.md`](../docs/architecture/crates-and-plugins.md).

## Rules

- **Reusable plugins import nothing from Waypoint.** No `waypoint-*` crate, no Waypoint concepts in code or docs. Domain plugins (`tauri-plugin-waypoint-*`) are thin adapters over the crates in [`crates/`](../crates).
- **Plugins never call each other.** Cross-concern behaviour is composed in the app through traits.
- **JavaScript uses `guest-js`.** Every command and event the front end uses has a typed function in the plugin's `guest-js`, which owns the command and event names. Nothing outside the plugin calls `invoke('plugin:…')`; `scripts/check-plugin-boundaries.sh` enforces it.
- **Every plugin reports availability** through `get_status`, so the UI can hide what does not work on the current system.
- **Per-platform code lives in platform modules** (`linux.rs`, `windows.rs`, `macos.rs`) with an unsupported fallback, not scattered `#[cfg]` inside logic.
- **Generated types name their own output.** Each `ts-rs` type uses `export_to`, and the generated bindings are committed under `guest-js/bindings`.
- **Every source file carries the licence header** described in [`AGENTS.md`](../AGENTS.md).

## Plugins

| Plugin                                   | Tier     | Status | What it is                                                                                                                                                                                                                                                  |
| ---------------------------------------- | -------- | ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [`system-appearance`](system-appearance) | Reusable | Built  | Reads the OS's titlebar preferences (which window buttons appear, where, and what double-click, middle-click and right-click do) on GNOME, KDE, Cinnamon, MATE, Xfce, Windows and macOS, and pushes changes. See its [README](system-appearance/README.md). |
| [`window-manager`](window-manager)       | Reusable | Built  | Reports which window manager features work (such as always-on-top) and asks the compositor to show its own window menu, which on Wayland is the only way to reach "Always on Top". See its [README](window-manager/README.md).                              |
| [`waypoint-session`](waypoint-session)   | Domain   | Built  | Each window's tabs, their order, the active tab and per-tab history, with granular events and a tab-close hook, over the `waypoint-session` crate. See its [README](waypoint-session/README.md).                                                            |

More reusable and domain plugins are planned; add one only when a milestone needs it.

## Graduating a plugin

Copy the plugin folder into `tauri-plugins-workspace/plugins/`, add it to that workspace's `members`, run `pnpm install`, and register it in `.changes/config.json` (a Rust and a JavaScript entry). The layout, `workspace = true` metadata and packaging already match that repository, and a dry run of `system-appearance` passed `cargo clippy -D warnings`, `cargo test` and `pnpm build` there. Two things differ on purpose:

- **`prepare` script.** The shared template has `"prepare": "pnpm build"`. It is left out here because `bun install` runs it and CI has no pnpm; the app builds the plugins through its own pre-hooks. Restore it when moving.
- **Prettier `trailingComma`.** This repository uses `all`, the shared one `es5`; run their `prettier --write` after copying.

Then switch Waypoint from the workspace path dependency to a versioned or tagged one and delete the in-repo copy.

## Working on the plugins

```bash
cargo test --workspace                          # Rust tests; regenerates each plugin's TypeScript bindings
bun run --cwd plugins/system-appearance build   # builds the guest-js package (also runs before app dev, build and tests)
```
