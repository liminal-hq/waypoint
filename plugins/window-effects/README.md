# @liminal-hq/plugin-window-effects

Reports which window effects work on this system (transparency, background blur, Mica, Acrylic and the shadow inset) and applies them to a window by its label. On Windows it uses Tauri's `set_effects`, and the Windows build number to know whether Mica and Acrylic exist. On Linux it asks the compositor to blur behind the window through `ext_background_effect_manager_v1` or KDE's blur protocol on Wayland, or the `_KDE_NET_WM_BLUR_BEHIND_REGION` property on X11, and tells the compositor how much of the window is shadow with `gdk_window_set_shadow_width`.

The plugin knows nothing about the app around it. It reports what works through `getStatus()`, so an interface can hide what does not. An effect the system cannot do is refused with a typed `unsupported` error and never panics.

How see-through a window is stays the page's own alpha (the window has to be created transparent): the effect shows only where the page is transparent, and `opacity` in the status only says whether the window can be transparent at all.

## Installation

### Rust

```toml
[dependencies]
tauri-plugin-window-effects = "0.1"

# Alternatively with Git:
tauri-plugin-window-effects = { git = "https://github.com/liminal-hq/tauri-plugins-workspace", branch = "main" }
```

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_window_effects::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Grant the windows that use it `window-effects:default` in a capability file, or the individual `window-effects:allow-get-status`, `window-effects:allow-apply`, `window-effects:allow-clear` and `window-effects:allow-set-shadow-inset`.

### JavaScript

```bash
pnpm add @liminal-hq/plugin-window-effects
```

## Usage

```typescript
import {
	apply,
	clear,
	getStatus,
	hasFeature,
	isWindowEffectsError,
	setShadowInset,
} from '@liminal-hq/plugin-window-effects';

const status = await getStatus();
const kind = hasFeature(status, 'mica') ? 'mica' : hasFeature(status, 'blur') ? 'blur' : 'none';

// Apply again whenever the theme changes.
await apply('main', { kind, dark: true });

// Blur only a region (Linux), in the window's logical pixels; apply again after a resize.
await apply('main', {
	kind: 'blur',
	dark: true,
	region: [{ x: 0, y: 0, width: 240, height: 600 }],
});

try {
	await apply('main', { kind: 'acrylic', dark: false });
} catch (error) {
	if (isWindowEffectsError(error) && error.kind === 'unsupported') {
		show(error.message); // error.reason is a code such as 'needs-windows-10'
	}
}

await setShadowInset('main', { top: 8, right: 8, bottom: 8, left: 8 }); // the margin the page draws its shadow in
await clear('main');
```

### Rust

```rust
use tauri_plugin_window_effects::{EffectKind, Effects, WindowEffectsExt};

app.window_effects()
    .apply("main", Effects { kind: EffectKind::Blur, dark: true, region: None })
    .await?;
```

## API

| Function                                                      | What it does                                                                                                                                                                                                                  |
| ------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `getStatus()`                                                 | `{ available, reason, message, flavour, features }`; see below. It reads the system again each time, because the compositor can change under the app.                                                                         |
| `apply(label, effects)`                                       | Puts `{ kind, dark, region? }` behind the window with that label, replacing the one it had. `kind` is `none` (the same as `clear`), `blur`, `mica` or `acrylic`. `dark` picks the dark Mica and the tint of Blur and Acrylic. |
| `clear(label)`                                                | Takes the window's effect away. A window with none is left as it is.                                                                                                                                                          |
| `setShadowInset(label, { top, right, bottom, left })`         | Tells the compositor that much of the window, in logical pixels, is shadow or invisible border, so a half-tiled or snapped window sits flush. All zeros puts it back.                                                         |
| `hasFeature(status, name)`, `featureReason`, `featureMessage` | Read the status.                                                                                                                                                                                                              |

A `region` is a list of rectangles `{ x, y, width, height }` in the window's logical pixels, each with a positive size and inside the window; leave it out for the whole window. A bad region or inset is refused before anything is changed. Windows effects always cover the whole window and ignore a region. The effect is not resized with the window: apply again after a resize when a region is set.

Errors are `WindowEffectsError` objects with a `kind`: `unsupported` (with a `reason` code and a `message`; nothing was changed), `windowNotFound` (with the `label`), `invalidRegion`, `invalidInsets` and `failed`.

## Features and status

`getStatus()` returns `{ available, reason, message, flavour, features }`, where each feature is `{ name, available, reason, message }`. Decide behaviour from the features, never from the platform. The top-level `reason` is the first missing feature's, leaving out features that belong to another platform (`windows-only`, `gtk-only`), so it is empty when everything that applies works.

| Feature       | Meaning                                                                                                                           |
| ------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| `opacity`     | A window can be transparent: compositing is on (always on Wayland and Windows).                                                   |
| `blur`        | The compositor blurs behind the window when asked.                                                                                |
| `mica`        | The Windows 11 Mica material (build 22000 or later).                                                                              |
| `acrylic`     | The Acrylic material (Windows 10 version 1809 or later). It can lag while the window is resized on some builds, so it is its own. |
| `shadowInset` | The compositor can be told how much of the window is shadow (GTK).                                                                |

`flavour` is `wayland`, `x11`, `windows` or `unsupported`. A `reason` is a code to branch on and `message` a sentence for people:

| Reason                   | Meaning                                                                                       |
| ------------------------ | --------------------------------------------------------------------------------------------- |
| `compositor-has-no-blur` | GNOME (Mutter) and Cinnamon (Muffin) give apps no way to blur behind their windows.           |
| `no-blur-protocol`       | A Wayland compositor with neither `ext_background_effect_manager_v1` nor KDE's blur protocol. |
| `unknown-compositor`     | An X11 desktop whose compositor is not known to honour the blur property (only KDE is).       |
| `x11-no-compositor`      | X11 with no compositing manager, so windows cannot be transparent.                            |
| `needs-windows-11`       | Mica needs build 22000 or later.                                                              |
| `needs-windows-10`       | Acrylic needs Windows 10 version 1809 (build 17763) or later.                                 |
| `windows-only`           | Mica and Acrylic are Windows materials.                                                       |
| `gtk-only`               | The shadow inset is a GTK feature.                                                            |
| `unsupported-platform`   | This operating system has no window effects in the plugin.                                    |
| `probe-failed`           | The system would not say what it can do (for example, the Windows build number).              |

## Platform notes

| Platform | Support                                                                                                                                                                                                                                                                                                                                |
| -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Windows  | Mica, Acrylic and Blur through `set_effects`, the theme chosen by `dark` and not by the system. Tauri does not report DWM's own failure, so `apply` succeeds once the request is made. The shadow inset is unavailable. Type-checked with `cargo xwin`; `live_mica` (ignored) checks the build number mapping in a Windows 11 machine. |
| Wayland  | Blur through the standard protocol (preferred, when its `capabilities` event says blur) or KDE's, on GTK's own `wl_surface` and connection. GNOME has neither and reports `compositor-has-no-blur`. The window must be shown before an effect can be put on it.                                                                        |
| X11      | Blur on KDE through `_KDE_NET_WM_BLUR_BEHIND_REGION` (in device pixels). Without a compositing manager nothing is transparent. XWayland counts as X11.                                                                                                                                                                                 |
| Other    | Every feature is unavailable with `unsupported-platform`.                                                                                                                                                                                                                                                                              |

The Wayland and X11 blur and the shadow inset call into GTK and the windowing system on the main thread, so they need a running GTK application. The pure parts (the status for each compositor, the region and inset checks, the mapping of a request to a Windows material) are tested without one, and the commands are tested over a fake backend. `cargo test -p tauri-plugin-window-effects --test live -- --ignored --nocapture` prints the real status of the running desktop (Linux) or checks the build number mapping (Windows); both only read.

## Licence

Apache-2.0 OR MIT
