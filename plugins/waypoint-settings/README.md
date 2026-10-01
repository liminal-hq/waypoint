# tauri-plugin-waypoint-settings

Waypoint's settings plugin: it holds the application settings that are not the operations settings as one `Settings` document over the `waypoint-settings` crate, saves it through a storage the app injects, and tells every window about every change.

This is a domain plugin, private to Waypoint (see `docs/architecture/crates-and-plugins.md`). Its JavaScript API is the `@liminal-hq/waypoint-plugin-settings` package in `guest-js/`; the app reaches the plugin only through it, and its wire types come from `@liminal-hq/waypoint-protocol`.

## How it works

- **One document, one writer.** `SettingsStore<R>` (Tauri state) holds the `SettingsSnapshot { revision, settings }`. `set_settings` validates the whole document, saves it, then makes it current, sends the event and runs the change hooks, all under one writer lock, so two changes never interleave. A change that fails validation or cannot be saved changes nothing. Setting what is already in force is quiet: no revision, no event.
- **Operations settings are not here.** `OpsSettings` (concurrency, verification, confirm before Trash, undo depth, Trash expiry) stay owned by `tauri-plugin-waypoint-ops`; the Settings window shows both and edits each through its owner.
- **Commands:** `get_status`, `get_settings` and `set_settings`. A rejected command is `{ kind: "invalid" | "storage", message }`, plus `field`, `min` and `max` for `invalid`, so the page can put the refusal under the row that caused it.
- **Event:** `waypoint-settings://changed` (`SETTINGS_EVENT`, `onSettingsChanged` in `guest-js`) goes to every window with the new `SettingsSnapshot`. The revision starts at 0 for what was loaded at start-up and grows by one per change; read `getSettings()` first and apply events with a higher revision.
- **Injected storage.** `init(storage)` or `init_with(|app| storage)` (made when the plugin is set up, so a file opened through `tauri-plugin-store` can be used). `waypoint_settings::Persistence` over a `KeyValue` file keeps the document under `settings` with the run before under `settingsPrevious`, and sets a file it cannot read aside as `…corrupt-{unix}` (`apps/waypoint/src-tauri/src/settings.rs`). Settings that cannot be loaded are never fatal: the defaults apply, and an out-of-range stored value is clamped.
- **Change hooks.** The composition root calls `SettingsStore::on_change(|settings| …)` to push what other state depends on (the view a new window starts with, in the session) without this plugin knowing about it. Hooks run after the new settings are readable and outside the plugin's state lock.

## Capabilities

The `settings` window may call all three commands; the main windows may call `get_status` and `get_settings` only; every other window none (`capabilities/settings.json`, `capabilities/main.json`).
