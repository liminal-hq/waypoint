# @liminal-hq/plugin-window-manager

Lets a frameless app reach window manager features a webview cannot: it reports which are supported, and asks the compositor to show its own window menu. On GNOME and other Wayland desktops that menu holds working "Always on Top", workspace moves and more, which an app cannot do for itself there.

## Installation

### Rust

```toml
[dependencies]
tauri-plugin-window-manager = "0.1"

# Alternatively with Git:
tauri-plugin-window-manager = { git = "https://github.com/liminal-hq/tauri-plugins-workspace", branch = "main" }
```

### JavaScript

```bash
pnpm add @liminal-hq/plugin-window-manager
```

## Usage

### Rust

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_window_manager::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### JavaScript

```typescript
import {
	getAlwaysOnTop,
	getCapabilities,
	getStatus,
	onAlwaysOnTopChanged,
	showSystemWindowMenu,
} from '@liminal-hq/plugin-window-manager';

const status = await getStatus();
const capabilities = await getCapabilities();

if (capabilities.systemWindowMenu) {
	titlebar.addEventListener('contextmenu', async (event) => {
		event.preventDefault();
		await showSystemWindowMenu({ x: event.clientX, y: event.clientY });
	});
}
```

Every command acts on the window that invoked it. Subscribe to `onAlwaysOnTopChanged` first and then read `getAlwaysOnTop()`, so a change between the two cannot be missed.

```typescript
const unlisten = await onAlwaysOnTopChanged((alwaysOnTop) => pin.setPressed(alwaysOnTop));
pin.setPressed((await getAlwaysOnTop()) ?? false);
```

`getAlwaysOnTop()` reads the state from the window manager instead of echoing the last request, which Tauri's `isAlwaysOnTop()` does on Linux. It resolves to `null` where the state cannot be observed (Wayland, and targets without an integration), so fall back to your own record of the last request there. On X11 the window manager's own menu can change it, so `onAlwaysOnTopChanged` reports those changes too; it never fires on other platforms. `showSystemWindowMenu` resolves to `true` if the compositor accepted the request, and to `false` if it is unsupported or was refused; it does not reject for "unsupported". Wayland only honours the request for a recent real button press, because the compositor validates the serial of that press, so call it straight from a pointer event handler. `position` is in CSS pixels from the window's top-left corner; the webview fills the window, so any transparent margin the app draws counts.

## Platforms

| Platform      | `session`   | `alwaysOnTop` | `systemWindowMenu`                    | Notes                                                                                                                                                                                             |
| ------------- | ----------- | ------------- | ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Linux Wayland | `'wayland'` | `false`       | `true`                                | `set_always_on_top` is a no-op under Wayland, so the compositor's window menu is the way to do it. Uses `xdg_toplevel.show_window_menu` through GDK.                                              |
| Linux X11     | `'x11'`     | `true`        | `true`                                | Sends `_GTK_SHOW_WINDOW_MENU` through GDK. Reads `_NET_WM_STATE_ABOVE` from the window manager, and reports changes made from its own menu.                                                       |
| Windows       | `'windows'` | `true`        | `true`                                | Shows the Windows system menu (`GetSystemMenu` and `TrackPopupMenu`) and sends the chosen command back with `WM_SYSCOMMAND`. Type-checked against the Windows target, but not yet run on Windows. |
| macOS         | `'macos'`   | `true`        | `true` when the app has a Window menu | A frameless window has no system menu on macOS, so this pops up the application's Window menu at the pointer. Type-checked against the macOS target, but not yet run on macOS.                    |
| Other         | `'unknown'` | `false`       | `false`                               | Targets without an integration, such as Android and iOS, offer nothing, so `getStatus()` reports the plugin unavailable.                                                                          |

On Linux the session is whatever GDK opened (`GdkWaylandDisplay` or `GdkX11Display`) when it can be asked on the GTK main thread. Otherwise it comes from the environment: `XDG_SESSION_TYPE` of `wayland` or `x11` decides; failing that a set `WAYLAND_DISPLAY` means Wayland (GTK prefers it even when `DISPLAY` is set for XWayland), then a set `DISPLAY` means X11; anything else is `'unknown'`.

## Types

All JSON is camelCase. TypeScript bindings are generated by ts-rs into `guest-js/bindings`.

```typescript
type Session = 'wayland' | 'x11' | 'windows' | 'macos' | 'unknown';

interface WindowCapabilities {
	session: Session;
	alwaysOnTop: boolean;
	systemWindowMenu: boolean;
}

// CSS pixels relative to the window's top-left corner.
interface WindowPosition {
	x: number;
	y: number;
}

// `features` names what works: 'always-on-top' and 'system-window-menu'.
interface PluginStatus {
	available: boolean;
	reason: string | null;
	features: string[];
}
```

## Permissions

The `default` permission set allows all four commands. Registering the plugin is not enough: Tauri denies every command until a capability grants it, so each window that uses the plugin needs the permission in a capability file, for example `src-tauri/capabilities/default.json`:

```json
{
	"identifier": "default",
	"windows": ["main"],
	"permissions": ["window-manager:default"]
}
```

Without it, every JavaScript call rejects with a "not allowed" error. Grant it only to the windows that need it; a wildcard scope also covers windows you add later.

| Permission                      | Command                   |
| ------------------------------- | ------------------------- |
| `allow-get-status`              | `get_status`              |
| `allow-get-capabilities`        | `get_capabilities`        |
| `allow-show-system-window-menu` | `show_system_window_menu` |
| `allow-get-always-on-top`       | `get_always_on_top`       |

## Development

Session detection is a pure function in `src/session.rs` with table-driven tests. The per-platform code is in `src/linux.rs`, `src/windows.rs`, `src/macos.rs` and `src/unsupported.rs`. The X11 Always on Top watcher and read are verified under XWayland on GNOME: a change made from outside the app (an `xdotool windowstate` request, the same message a window manager menu sends) produces the event and updates the read. To regenerate the TypeScript bindings, run `cargo test` in the plugin directory; to print the capabilities detected on this machine, run `cargo test live_capabilities -- --ignored --nocapture`. Whether the compositor actually shows the menu can only be checked by hand with a real click. Verified on GNOME (Mutter) under Wayland: choosing an entry in an app-drawn menu shows GNOME's own window menu and its Always on Top works, alongside Always on Visible Workspace and workspace and monitor moves. Other compositors, X11, and a separate check on KDE are still to do.

## Licence

Licensed under either of Apache License 2.0 or MIT licence at your option.

## Logging

The plugin logs through the `log` crate, so `tauri-plugin-log` captures it with the rest of the app's output. The Windows and macOS menu paths call into Win32 and AppKit and have only been type-checked, so each native step logs at `info` before it runs, with the window's `label=`. A line containing `untested native path` marks them. If the process crashes inside one of those calls, the last such line in the log names the step that was running. Failures that are not crashes log at `warn`.
