# @liminal-hq/plugin-native-dnd

OS-level drag and drop of files for a Tauri app, and the file clipboard:

- **Inbound:** turns the runtime's drag-drop events into `enter`, `over`, `drop` and `leave` events with positions in webview CSS pixels, the modifier keys, lossless `file://` URIs, and a flag that marks the end of a drag the app started itself.
- **Outbound:** starts a drag of files out of a window to other applications (a GTK drag source on Linux, OLE `DoDragDrop` on Windows), so a pointer drag the page started can continue as an OS drag.
- **Clipboard:** copies and cuts files to the system clipboard in the format file managers use (`x-special/gnome-copied-files` and `text/uri-list` on Linux, `CF_HDROP` with a Preferred DropEffect on Windows), reads them back, and reports when the clipboard changes.

The plugin registers no drop target of its own. It listens to the drag-drop events Tauri already produces, so a window gets inbound events only if its drag-drop handler is on (the default; `disable_drag_drop_handler()` turns it off). Drags of any other type, such as an app's own custom MIME type, pass untouched. It knows nothing about what the files are for.

## Installation

### Rust

```toml
[dependencies]
tauri-plugin-native-dnd = "0.1"

# Alternatively with Git:
tauri-plugin-native-dnd = { git = "https://github.com/liminal-hq/tauri-plugins-workspace", branch = "main" }
```

### JavaScript

```bash
pnpm add @liminal-hq/plugin-native-dnd
```

## Usage

### Rust

```rust
use tauri_plugin_native_dnd::NativeDndExt;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_native_dnd::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// Elsewhere, with an `AppHandle`:
// let status = app.native_dnd().status().await;
// app.native_dnd().set_files(ClipboardFiles { uris, cut: false }).await?;
```

Grant a window the `native-dnd:default` permission in a capability file to let its page call the commands. The inbound events are sent to a window without a grant.

### JavaScript

```typescript
import {
	getStatus,
	hasFeature,
	isNativeDndError,
	onDragEnded,
	onDrop,
	onEnter,
	onLeave,
	onOver,
	setFiles,
	startDrag,
} from '@liminal-hq/plugin-native-dnd';

const status = await getStatus();
if (hasFeature(status, 'outbound')) {
	// Call from a pointer handler while the primary button is down, once the pointer has left the window.
	try {
		await startDrag({ uris, actions: ['copy', 'move'] });
	} catch (error) {
		if (isNativeDndError(error, 'buttonNotPressed')) {
			/* keep the in-page drag */
		}
	}
}

await onDragEnded(({ outcome }) => {
	/* reset the page's own pointer state; if outcome is 'dropped-move', remove the originals */
});

await onEnter(({ uris, position, modifiers }) => {});
await onOver(({ position }) => {});
await onDrop(({ uris, selfDrop }) => {});
await onLeave(() => {});

// From a key or click handler, so Wayland accepts it:
await setFiles({ uris, cut: false });
```

### Commands

| Command                   | What it does                                                                                                                    |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| `get_status`              | Which features work, and why the others do not                                                                                  |
| `start_drag`              | Starts an outbound drag of `uris` with the allowed `actions` (`copy`, `move`, `link`) and an optional PNG `icon`; one at a time |
| `set_files` / `get_files` | Writes and reads the file clipboard                                                                                             |

Failures reject with `{ kind, message }`; `kind` is `unsupported`, `buttonNotPressed`, `alreadyActive`, `invalid` or `failed`.

### Events

| Event                            | Payload                                                              |
| -------------------------------- | -------------------------------------------------------------------- |
| `native-dnd://enter`             | `{ window, paths, uris, position, modifiers, action }`               |
| `native-dnd://over`              | `{ window, position, modifiers, action }`                            |
| `native-dnd://drop`              | `{ window, paths, uris, position, modifiers, action, selfDrop }`     |
| `native-dnd://leave`             | `{ window }`                                                         |
| `native-dnd://drag-ended`        | `{ id, outcome, uris, reason }`, to the window that started the drag |
| `native-dnd://clipboard-changed` | none, to every window                                                |

`uris` are the lossless form: every byte of a name outside `A-Za-z0-9-._~/` is percent-encoded, so a Linux name that is not valid UTF-8 survives and can be reopened. `paths` are for display only. Positions are logical pixels from the top-left of the webview on both platforms (the plugin converts Windows' physical pixels in one place).

## Features and availability

`get_status` reports `inbound`, `outbound`, `positions`, `modifiers`, `clipboard` and `self-drop-filter`, each with `available` and a `reason`, and the display server (`wayland`, `x11`, `windows` or `none`). With no display, and on other platforms, every feature is unavailable and the commands reject with `unsupported`.

## Platform notes

Findings from running the plugin on GNOME's Mutter (headless, Wayland and XWayland) and on Windows 11.

### Linux

- **Outbound needs no Wayland serial code.** GTK's drag from the webview widget carries the pointer-press serial itself. The command refuses with `buttonNotPressed` unless the primary button is down, because Wayland silently ignores a stale-serial drag and X11 would start a ghost drag with no button.
- **WebKit keeps believing the button is down** after GTK takes the release that ends the drag: the next click gets a `pointerup` with no `pointerdown` and a second drag does not start. The plugin sends a synthetic button release to the webview when the drag ends. Use `drag-ended` to reset the page's own drag state; the page gets no pointer events between the start and the end.
- **Wayland has no link action**, so `link` is ignored by the compositor. GDK reports Escape and a drop on nothing as the same error, so both end as `cancelled`.
- **Modifier keys during a drag:** X11 and Windows read them natively. A Wayland compositor takes the keyboard for the drag and tells the app nothing, so the events report all modifiers released and `modifiers` is unavailable in the status.
- **The negotiated action:** `enter`, `over` and `drop` carry `action` (`copy`, `move`, `link` or `null`). On Linux it is read from the drag context at the last motion, kept to the actions the source offers: on Wayland the selected action, which the compositor sets from the keys it holds (Mutter: Shift for move, Ctrl for copy), with the suggested action as the fallback before the compositor has answered; on X11 the suggested action, which GTK sets from the modifiers, with the selected action as the fallback. GTK 3 on Wayland keeps the suggested action at Copy whatever is held, so reading it there always gave a copy (#567); a move is never reported for a source that does not offer one, and ask is not reported. It is `null` on Windows (read `modifiers` there) and wherever nothing is negotiated. Wry's drag handler and WebKit answer every file drag with a copy of their own and the plugin does not claim the signal, so with no key held the action is a copy and the compositor's Shift still turns it into a move; the action is read at pointer motion, so a key changed with the pointer still shows at the next motion. The plugin never moves files: the app does, if it chooses.
- **Names that are not UTF-8:** wry's own `paths` are lossy for them. The plugin reads the raw `text/uri-list` in a second `drag-data-received` handler and builds `uris` from that.
- **Clipboard on Wayland:** the compositor ignores a selection from a client without a recent input serial, so call `set_files` from the handler of a key press or click. Reading another application's clipboard needs the app to have focus, and runs a nested main loop until the owner answers. The plugin keeps no copy after the app exits.

### Windows

- `DoDragDrop` is a modal loop, so `start_drag` resolves when the drag has finished and `drag-ended` is sent as well. Escape cancels; the drop happens when the button that started the drag is released. Windows draws the drag image, so `icon` is not used.
- **Hover feedback in another window of the same process.** `DoDragDrop` runs on the main thread inside the event loop's handler, so the webview's own drag events for a window of this process (a second window, a Shelf-like drop target) are held until it returns: the target would show nothing until the button is released and then replay the whole movement. While the loop runs, the drop source reads the cursor on every query, finds the window of this application under it (`WindowFromPoint`, the top-level window mapped to its label, ignoring the shell's drag image) and sends that window the plugin's own `enter`, `over` and `leave` with the offered files, the cursor in the window's client area and the keyboard modifiers. The held-back `enter`, `over` and `leave` for a window already told are swallowed when they arrive; its `drop` is always delivered, so a page sees one drag begin and one end. A release over a window that then gets no drop is given up on after 1.5 seconds and the window is sent `leave`. Escape sends `leave` at once. Drags from other applications are unaffected, since their loop is not this one. Unverified by hand on a real pointer drag; the decisions are the pure `hover` module's and are unit tested.
- The drop reaches the page after `DoDragDrop` has returned, so a drop of the same files counts as `selfDrop` for 1.5 seconds after an outbound drag ends.
- The clipboard holds the files as `CF_HDROP` with a `Preferred DropEffect` of 1 (copy) or 2 (cut). The plugin flushes it when the app exits, so a paste still works afterwards. Clipboard changes come from `AddClipboardFormatListener` on a hidden message window. A busy clipboard (another process has it open) is retried briefly.
- A name with an unpaired surrogate cannot be written as UTF-8 and comes back with a replacement character.

## Graduation checklist

- [x] No imports from, or mentions of, the app that incubated it; `scripts/check-plugin-boundaries.sh` enforces this.
- [x] `get_status`, `permissions/default.toml` with every command, `guest-js` for every command and event, `ts-rs` bindings under `guest-js/bindings`, platform modules with an `unsupported` fallback.
- [x] Pure logic (URI codec, position and modifier mapping, the outbound state machine, status) unit tested without a display; mock-runtime plugin tests.
- [ ] Copy into `tauri-plugins-workspace/plugins/`, add it to the workspace `members`, register it in `.changes/config.json`, restore the template's `prepare` script, run the shared Prettier.
- [ ] Verify against real Nautilus and Dolphin, and a real pointer drag onto Explorer on Windows 11.

## Testing

```bash
cargo test -p tauri-plugin-native-dnd          # unit and mock-runtime tests; regenerates the bindings
cargo xwin check --target x86_64-pc-windows-msvc -p tauri-plugin-native-dnd
```

The Windows live tests are ignored by default (`cargo test ... --lib -- --ignored`) because they use the real clipboard.

## Licence

Apache-2.0 OR MIT.
