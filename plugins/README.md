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

More reusable and domain plugins are planned; add one only when a milestone needs it.

## Working on the plugins

```bash
cargo test --workspace                          # Rust tests; regenerates each plugin's TypeScript bindings
bun run --cwd plugins/system-appearance build   # builds the guest-js package (also runs before app dev, build and tests)
```
