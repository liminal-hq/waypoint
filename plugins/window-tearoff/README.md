# @liminal-hq/plugin-window-tearoff

Lets an app drag something out of a window. A small click-through "ghost" window follows the cursor from the native side, so the drag stays continuous after the pointer leaves the webview, and when the drag ends the plugin reports which of the app's own drop regions the cursor was over, across every window and monitor. The ghost carries an opaque JSON payload that the app defines and the ghost page draws.

The plugin knows nothing about what is dragged. It moves a payload, follows a cursor and answers "which region is under this point".

## Installation

### Rust

```toml
[dependencies]
tauri-plugin-window-tearoff = "0.1"

# Alternatively with Git:
tauri-plugin-window-tearoff = { git = "https://github.com/liminal-hq/tauri-plugins-workspace", branch = "main" }
```

### JavaScript

```bash
pnpm add @liminal-hq/plugin-window-tearoff
```

## Usage

### Rust

```rust
use tauri_plugin_window_tearoff::Options;

fn main() {
    let options = Options {
        ghost_label: "tear-ghost".into(),
        ghost_url: "index.html".into(),
        ghost_size: (240.0, 80.0),
    };
    // A window-state plugin shows any window it has no saved state for, which defeats the hidden ghost, and saves the ghost's state at exit. Tell it to ignore the ghost.
    let denylist = options.window_state_denylist();

    tauri::Builder::default()
        .plugin(tauri_plugin_window_tearoff::init(options))
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

`init` creates the one shared ghost as soon as the event loop starts (not in `setup`, where Tauri's locked plugin store would deadlock a window build): hidden, transparent, undecorated, resizable, always on top, non-focusable, off the taskbar, without a shadow and without the file-drop handler. It then makes the ghost click-through on the main thread, after realising it, because making a window that was never shown click-through aborts the process on Linux. Grant the ghost window the plugin's `default` permission (and `core:event:default`) in a capability file; the ghost must be resizable because a non-resizable window ignores `set_size` on Linux.

### JavaScript

In the window the drag starts from:

```typescript
import {
	begin,
	end,
	getStatus,
	hasFeature,
	setDropRegions,
} from '@liminal-hq/plugin-window-tearoff';

// At startup: the first call probes the windowing system, which can take up to a second.
const status = await getStatus();

// Regions are logical pixels from the top-left of the window's content.
await setDropRegions([{ id: 'tab-strip', x: 0, y: 0, width: 800, height: 40 }]);

const started = await begin(
	{ title: 'Documents', count: 1 },
	{ x: 40, y: 14 },
	{ width: 220, height: 56 },
);
if (started.state === 'noGhost') {
	// Draw a preview inside the page instead.
}

// On release (or `'cancel'` on Esc):
const report = await end('drop');
if (report.hit) {
	// report.hit.window and report.hit.region name the region under the cursor.
} else if (report.cursor) {
	// Outside every region: open a new window at report.cursor (physical pixels, divide by report.scaleFactor for logical).
}
```

In the ghost window, draw whatever the payload says:

```typescript
import { getPayload, onPayload } from '@liminal-hq/plugin-window-tearoff';

const unlisten = await onPayload((payload) => render(payload));
render(await getPayload()); // the page may have loaded after `begin`
```

`begin` places the ghost at `cursor - grabOffset × scaleFactor`, using the calling window's own scale factor, so the ghost lands where the dragged thing was grabbed. It returns `{ state: 'following' }`, `{ state: 'noGhost' }` where no ghost can be shown (nothing was started, but `end` still reports the cursor and hit where it can), or `{ state: 'alreadyActive' }`. The follow thread moves the ghost every 16 ms and ends a drag that runs 30 seconds with a `window-tearoff://timeout` event to the window that began it. Only one drag runs at a time.

`hitTest()` returns the region under the cursor right now without ending the drag, so a caller can change the ghost's label to say what a release would do; it is `null` where the cursor or hit-testing is unavailable.

`end` hides the ghost and returns a `DropReport`: the cursor in physical pixels (or `null` where the system reports none), the source window's scale factor, whether the cursor value had gone stale, and the hit. A cancelled drag never has a hit. A drag belongs to the window that began it: another window's `end` still reports the cursor and hit but leaves the drag running. Ending a drag (by `end`, the timeout or the source window closing) also clears the payload and sends the ghost a `null` payload, so the ghost page can clear its card; an `end` that arrives while `begin` is still showing the ghost cancels that `begin`, which then reports `noGhost`.

## Features and status

`getStatus()` returns `{ available, reason, features, unavailable }`. `features` lists what works; `unavailable` lists each feature that does not, with the reason. Decide behaviour from the features, never from the platform.

| Feature           | Meaning                                                                                                                                                              |
| ----------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ghost`           | The ghost window exists and can be placed under the cursor.                                                                                                          |
| `cursor_follow`   | The native cursor is live while a button is held. True on X11 (including XWayland) and Windows. False on Wayland, where the toolkit reports `(0, 0)` for every read. |
| `window_position` | Probed at runtime: the ghost is asked for two positions and the plugin checks that the reads move the same way. Wayland ignores positions and reads `(0, 0)`.        |
| `hit_test`        | Probed at runtime: windows report readable inner positions, so a cursor can be tested against their regions.                                                         |

The probes run once per session and are cached. A platform claim is not enough for the cursor: under XWayland it follows the pointer only while a button is held or the pointer is over one of the app's windows, and otherwise stays at its last value without an error. The follow loop therefore watches the value, and when it stops changing for a second while a button is held it stops moving the ghost and sends `window-tearoff://cursor-stale` (`true`); when it changes again it resumes and sends `false`. `DropReport.cursorStale` says whether it was stale at the end.

Hit-testing uses each window's inner (content) position and size, never its outer position: a frameless window with a drop shadow has an outer frame above and left of its content. The windowing system does not report stacking order, so where regions overlap the one with the smallest visible area wins, then the earlier window by label, then the earlier region.

## Platforms

| Platform      | `ghost` | `cursor_follow` | `window_position` | `hit_test` | Notes                                                                                                                                                                                                                                                                                                              |
| ------------- | ------- | --------------- | ----------------- | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Linux X11     | probed  | `true`          | probed            | probed     | XWayland counts as X11. Shows the ghost before positioning it, because a position set before `show` is overridden at map time.                                                                                                                                                                                     |
| Linux Wayland | `false` | `false`         | `false`           | `false`    | No cursor, no window positions. Take the in-page preview path.                                                                                                                                                                                                                                                     |
| Windows       | `true`  | `true`          | `true`            | `true`     | Reported without probing. Run on Windows 11 in the milestone 3 verification. Every ghost show is `SW_SHOWNOACTIVATE`: `tao` only does that for a window's first show, and a later `SW_SHOW` would activate the ghost despite `WS_EX_NOACTIVATE`, so the source window would blur and Escape would reach the ghost. |
| Other         | `false` | `false`         | `false`           | `false`    | macOS, Android and iOS report the plugin unavailable.                                                                                                                                                                                                                                                              |

## Types

All JSON is camelCase. TypeScript bindings are generated by ts-rs into `guest-js/bindings`.

```typescript
interface Point {
	x: number;
	y: number;
}
interface Size {
	width: number;
	height: number;
}
interface Region {
	id: string;
	x: number;
	y: number;
	width: number;
	height: number;
}
type Outcome = 'drop' | 'cancel';
type BeginState = 'following' | 'noGhost' | 'alreadyActive';
interface BeginReport {
	state: BeginState;
}
interface Hit {
	window: string;
	region: string;
}
interface DropReport {
	cursor: Point | null;
	scaleFactor: number;
	cursorStale: boolean;
	hit: Hit | null;
}
interface UnavailableFeature {
	feature: string;
	reason: string;
}
interface PluginStatus {
	available: boolean;
	reason: string | null;
	features: string[];
	unavailable: UnavailableFeature[];
}
```

## Events

| Event                           | Sent to                      | Payload                                    |
| ------------------------------- | ---------------------------- | ------------------------------------------ |
| `window-tearoff://payload`      | the ghost window             | the payload, or `null` when the drag ended |
| `window-tearoff://timeout`      | the window that began a drag | none                                       |
| `window-tearoff://cursor-stale` | the window that began a drag | `boolean`                                  |

## Permissions

The `default` permission set allows every command. Tauri denies a command until a capability grants it, so each window that uses the plugin, including the ghost, needs the permission:

```json
{
	"identifier": "tearoff",
	"windows": ["main-*", "tear-ghost"],
	"permissions": ["window-tearoff:default"]
}
```

| Permission                              | Command            |
| --------------------------------------- | ------------------ |
| `window-tearoff:allow-get-status`       | `get_status`       |
| `window-tearoff:allow-begin`            | `begin`            |
| `window-tearoff:allow-update`           | `update`           |
| `window-tearoff:allow-end`              | `end`              |
| `window-tearoff:allow-set-drop-regions` | `set_drop_regions` |
| `window-tearoff:allow-get-cursor`       | `get_cursor`       |
| `window-tearoff:allow-get-payload`      | `get_payload`      |

## Licence

Apache-2.0 OR MIT.
