# @liminal-hq/plugin-os-prefs

Reports the user's operating system preferences that a webview cannot see, starting with the **12/24-hour clock**, and pushes a change event so open windows update without a restart.

`Intl.DateTimeFormat` and libc derive the hour cycle from the _locale_ — `en-CA` defaults to 12-hour — not from the user's actual setting, so an app on a machine set to 24-hour time shows "10:51 a.m." This plugin reads the real setting and tells you where the answer came from.

The same plugin also carries the Android-only helpers it started with: Developer Options' animator duration scale and a jump to the app's notification settings.

## Installation

### Rust

```toml
[dependencies]
tauri-plugin-os-prefs = "0.1"

# Alternatively with Git:
tauri-plugin-os-prefs = { git = "https://github.com/liminal-hq/tauri-plugins-workspace", branch = "main" }
```

### JavaScript

```bash
pnpm add @liminal-hq/plugin-os-prefs
```

## Usage

### Rust

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_os_prefs::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Grant the permission in a capability file. Listening to the change event also needs the core event permission:

```json
"permissions": ["os-prefs:default", "core:event:default"]
```

### JavaScript

```typescript
import {
	getStatus,
	getTimeFormat,
	hourCycleOf,
	onTimeFormatChanged,
} from '@liminal-hq/plugin-os-prefs';

const { is24Hour, source } = await getTimeFormat();

// Give Intl the user's real hour cycle; `undefined` leaves the choice to the locale.
let hourCycle = hourCycleOf(await getTimeFormat());
const format = () =>
	new Intl.DateTimeFormat('en-CA', { dateStyle: 'medium', timeStyle: 'short', hourCycle });
console.log(format().format(new Date())); // "Oct 1, 2026, 13:05"

const unlisten = await onTimeFormatChanged((next) => {
	hourCycle = hourCycleOf(next);
});

const status = await getStatus();
```

`getTimeFormat()` never rejects because a source is unavailable. When nothing can be read it answers `{ is24Hour: false, source: 'default' }`; treat `source: 'default'` as "unknown" and let `Intl` decide (`hourCycleOf` does this by returning `undefined`).

The plugin remembers the last answer it gave and emits `os-prefs://time-format-changed` with the new `{ is24Hour, source }` only when the 12/24-hour answer flips. To avoid missing a change made while starting up, subscribe first and read second.

## Commands and events

| Name                             | Kind    | Payload                                    |
| -------------------------------- | ------- | ------------------------------------------ |
| `get_status`                     | command | `PluginStatus`                             |
| `get_time_format`                | command | `{ is24Hour: boolean, source: string }`    |
| `get_animator_duration_scale`    | command | `{ scale: number }` (always 1 off Android) |
| `open_notification_settings`     | command | nothing (Android only; a no-op elsewhere)  |
| `os-prefs://time-format-changed` | event   | `{ is24Hour: boolean, source: string }`    |

`source` is one of `gnome-portal`, `gnome-gsettings`, `cinnamon-gsettings`, `locale`, `windows-user-locale`, `windows-fallback`, `macos-date-template`, `ios`, `android` or `default`.

`getStatus()` answers `{ available, reason, features, timeFormatSource }`. `features` lists `timeFormat`, `timeFormatWatch`, `animatorDurationScale` and `notificationSettings`, each as `{ name, available, reason }`; `reason` explains why a feature is missing, or in which way it is degraded (for example `timeFormat` on KDE is available but notes that the locale's convention is used). Hide settings UI for features that report `available: false`.

## Sources by platform

| Platform | Desktop                | Source                                                                                                                                                                                                                                                    | Pushed when it changes                                      |
| -------- | ---------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------- |
| Linux    | GNOME family           | xdg-desktop-portal Settings: `org.gnome.desktop.interface` `clock-format` (`gnome-portal`), then `gsettings get` of the same key (`gnome-gsettings`)                                                                                                      | The portal `SettingChanged` signal                          |
| Linux    | Cinnamon               | `gsettings get org.cinnamon.desktop.interface clock-use-24h` (`cinnamon-gsettings`)                                                                                                                                                                       | `gsettings monitor`                                         |
| Linux    | KDE, Xfce, MATE, other | `nl_langinfo(T_FMT)` under the process's `LC_TIME` locale (`locale`)                                                                                                                                                                                      | Nothing; the value is read on demand                        |
| Windows  |                        | `GetLocaleInfoEx` on the user default locale: the short-time pattern (`LOCALE_SSHORTTIME`), then the long-time pattern, then `LOCALE_ITIME` (`windows-user-locale`); the system default locale if the user's settings cannot be read (`windows-fallback`) | A 5 second poll, deduplicated                               |
| macOS    |                        | `NSDateFormatter.dateFormatFromTemplate("j")` for the auto-updating locale, which honours System Settings' "24-Hour Time" (`macos-date-template`)                                                                                                         | `NSCurrentLocaleDidChangeNotification` plus a 5 second poll |
| iOS      |                        | The same Foundation hour pattern, read on demand (`ios`)                                                                                                                                                                                                  | Nothing                                                     |
| Android  |                        | `DateFormat.is24HourFormat` (`android`)                                                                                                                                                                                                                   | Nothing                                                     |

The Linux desktop comes from `XDG_CURRENT_DESKTOP`, a colon-separated list whose first recognised entry wins.

Why the portal is only consulted on GNOME-family desktops: `org.gnome.desktop.interface` belongs to GNOME, and on other desktops the GTK portal backend reports that schema's default (`24h`) whatever the user chose, which would override a correct locale. KDE applies its Region settings as `LC_TIME`; Xfce and MATE keep the clock format in per-panel plugin options with no stable key to read, so they use the locale's convention too. The libc read uses a private locale object built from `LC_ALL`/`LC_TIME`/`LANG`, so it does not depend on the host app having called `setlocale`.

### Limits

- On KDE, Xfce, MATE and other desktops a change to the setting is not pushed, and a locale change only reaches the process after the session restarts.
- This repository's CI runs on Linux only. The Windows, macOS and iOS readers are type-checked (`cargo xwin check --target x86_64-pc-windows-msvc`, `cargo clippy --target aarch64-apple-darwin` and `aarch64-apple-ios`); the parsing they depend on is pure and unit tested on every platform.
- Windows polls instead of listening for `WM_SETTINGCHANGE`: a message-only window does not receive that broadcast, so listening would need a hidden top-level window and its own message loop. A 5 second poll saw a Region change on Windows 11 with and without the broadcast. The Windows reader has been run on a Windows 11 VM (`cargo test -- --ignored --nocapture live_region_read`); the macOS and iOS readers have not been run on a device.
- iOS has no native Swift/Objective-C plugin: the time format is read from Rust, and the animator duration scale and notification settings are neutral no-ops.

## Migrating from Threshold's local `os-prefs`

The command names, the `is24Hour`/`scale` field names and the permission identifiers (`allow-get-time-format`, `allow-get-animator-duration-scale`, `allow-open-notification-settings`) are unchanged, so an app switches by changing its dependency only. The differences are additive or cosmetic:

- `get_time_format` replies with an extra `source` field, and `get_status` and the change event are new.
- The Android plugin class lives in the package `ca.liminalhq.osprefs` (it was `ca.liminalhq.threshold.osprefs`); the Rust side registers it, so nothing in an app refers to the name.
- On desktop and iOS the Rust API (`OsPrefsExt::os_prefs()`) is `async`, and `get_time_format` now reports the real setting instead of a fixed 12-hour answer. An app that relied on the old desktop stub's `false` and inferred 24-hour time from `Intl` should call `hourCycleOf()` instead.
- The npm package is `@liminal-hq/plugin-os-prefs` (it was `tauri-plugin-os-prefs-api`).

## Types

All JSON is camelCase except the `source` values, which are kebab-case. TypeScript bindings are generated by ts-rs into `guest-js/bindings`; run `cargo test` to regenerate them.

## Permissions

`os-prefs:default` allows `get_status`, `get_time_format`, `get_animator_duration_scale` and `open_notification_settings`. See `permissions/autogenerated/reference.md` for the individual permissions.

## Platform support

| Platform | Support                                                                             |
| -------- | ----------------------------------------------------------------------------------- |
| Linux    | Full on GNOME and Cinnamon; locale convention elsewhere                             |
| Windows  | Reads the Region settings and polls for changes; run on Windows 11                  |
| macOS    | Type-checked; reads the hour pattern, follows locale-change notifications and polls |
| Android  | Full: time format, animator duration scale, notification settings                   |
| iOS      | Time format only, read on demand                                                    |
