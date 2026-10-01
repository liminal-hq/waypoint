# Tauri Tab Tear-Off (implementation reference)

Reference: [romenkova/doska](https://github.com/romenkova/doska), by way of a Reddit post in r/tauri. It shows a clean way to drag something out of a Tauri window and turn it into a new window. Waypoint's tab tear-off should use the same pattern if we build on Tauri v2.

doska is MIT-licensed, so the pattern is reused and its code may be reused with attribution. This doc describes the pattern in our own words and does not reproduce their source.

## Why this pattern

A webview can only track the pointer while it is inside the window. Once a drag leaves the window, the page stops getting move events. The pattern fixes this with a small always-on-top "ghost" window that follows the cursor from the native side, so the drag still looks continuous outside the app.

## The three parts

### 1. Drag handle, in the web UI

- Mouse down on a marked handle starts a _pending_ drag. Ignore buttons, links and inputs inside the handle.
- Wait for a small movement threshold (4 px) before it counts as a drag. Below that it is still a click.
- Once it counts as a drag: clear any text selection, set `user-select: none` and a grabbing cursor on `body`, and ask the backend to show the ghost.
- While dragging, check whether the pointer is outside the window's viewport. If it is, fade the source (in Doska, opacity to about 40%).
- On mouse up: restore the body styles, tell the backend to hide the ghost, and if the pointer is outside the viewport, open the new window at the drop point.
- Remember where inside the handle the user grabbed (the offset). Subtract it from the screen coordinates on drop so the new window lands where the ghost was, not with its corner under the cursor.

### 2. Ghost window, in Rust

- A pre-created, hidden, small window (label like `tear-ghost`) that shows a thumbnail of the thing being dragged.
- `start_tear_off` command: guard with an atomic flag so it can't start twice, position the ghost under the cursor, apply the current theme, show it, and set it to ignore cursor events (so it never steals the pointer or blocks drop targets).
- A background thread nudges the ghost to the cursor about every 16 ms using the app's cursor position, run on the main thread. Centre the ghost on the cursor using its outer size.
- A hard timeout (about 30 s) ends the drag if the mouse-up is ever lost.
- `end_tear_off` command: clear the flag and hide the ghost.

### 3. New window, in the web UI

- On drop outside the viewport, create a new webview window with a stable label, its own route/URL, and the drop coordinates as its position.
- If a window for the same item already exists, show and focus it instead of making another.
- Set the window's background colour and theme from the current theme so it doesn't flash white.
- Turn off native drag-drop on the new window if the app handles file drops itself.
- Cap how many hidden windows linger and destroy the oldest. Doska hides on close (destroying a webview can crash WebKit on macOS), so this cap matters.

## How it maps to Waypoint

| Waypoint behaviour                           | Prototype                                                                   | Tauri build                                                                                |
| -------------------------------------------- | --------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| Tear a tab off                               | Drag more than 24 px away from the tab row; a ghost card follows the cursor | The handle logic above on the tab; ghost window follows the native cursor                  |
| Ghost content                                | A mini window card with the tab title                                       | The ghost window renders the same card; send the title and theme through the start command |
| Release outside                              | New floating window at the drop point                                       | New `WebviewWindow` at the drop point, loading the tab's location                          |
| Release on another Waypoint window's tab row | "Merge into that window"                                                    | Needs cross-window hit-testing (see below)                                                 |
| Drop files from the tab onto other windows   | Not covered                                                                 | Same ghost, plus an OS-level drop target on the other window                               |
| Esc cancels                                  | Yes                                                                         | End the drag and hide the ghost on Esc                                                     |

## How Waypoint wires it (milestone 3, slice 10)

The pieces above are `plugins/window-tearoff` (ghost, follow thread, regions, `hit_test`) and Waypoint's `apps/waypoint/src/tabs/tearOff.ts` (the new-window phase of the drag engine). What was run on this design:

- **Verified on X11 (XWayland under a private headless Mutter, a button held through the compositor's remote-desktop API):** `get_status` reports all four features; the ghost sits at `cursor − grab offset` and follows the cursor; `hit_test` finds another window's strip and its label changes to "Release to merge into …"; a release on that strip merges the tab (the source window closes when it was the last tab); a release elsewhere opens a window whose top edge is at the cursor less the grab point (a left edge that would run off the screen is held back by the window manager); the ghost is hidden again afterwards.
- **Verified on Wayland (the same headless compositor):** `get_status` reports every feature off with its reason, the ghost exists hidden and never shows, the in-page card and pill follow the pointer, a release opens a window the compositor places, Esc cancels, and Move to Window ▸ merges.
- **Not verified:** a real hardware pointer, KDE, Windows 11, a second monitor or mixed scale factors, Esc while a ghost is following (the injected key never reached an unfocused window), and the 30 s timeout path end to end (the hook handles `window-tearoff://timeout` in tests only).
- **Two traps found by running it.** The ghost cannot be built inside the plugin's `RunEvent::Ready` hook (Tauri holds its plugin store locked there, the same deadlock as `setup`); the plugin asks the main thread for it from the async runtime. And once the pointer leaves the page, the page gets no `pointermove` events (a `pointerup` still arrives), so anything that must change while the ghost follows (the merge label) runs on a timer against `hit_test`, not on pointer events. The `cursor-stale` event also fires for a pointer held still, so it is not treated as an error; only the drop report's `cursorStale` stops a window being placed.
- **Placement.** The new window's geometry is the source window's inner size and an inner origin of `cursor − (grab + frame margin) × scale` in physical pixels, so the visible window's corner sits at the grab point; `fit_geometry` on the Rust side drops a position no monitor holds.

## Wayland: a real window with `xdg-toplevel-drag` (milestone 3, slice 13)

Where the compositor offers `xdg_toplevel_drag_manager_v1` (GNOME's Mutter does; KDE has it too, unverified here), Waypoint does not draw a ghost: the compositor moves a real window. The app starts a `wl_data_device` drag and attaches a toplevel to it; the window follows the pointer outside every window, stays where it is dropped, and snaps back on cancel. The generic part is `crates/wayland-toplevel-drag` (see its README); the Tauri part is the `toplevel_drag` feature of `plugins/window-tearoff`; the flow is `tabs/tearOff.ts` and `tabs/tearOffHandoff.ts`.

**The flow.** The drag engine's first move past 24 px out of the strip starts it. The page flushes the tabs' hints, asks the app to keep the next window hidden (`hold_next_window`, read by the session's window factory), moves the tabs with `moveTabs(NewWindow)`, and calls `begin_toplevel_drag(payload, newLabel, grabOffset)`. The hold belongs to the window that set it (the factory is told which window a command came from), so a window another window opens in the meantime is not held; it applies to one window and lapses after 10 seconds, and the page releases it again once the move has returned, whatever the result. If `begin_toplevel_drag` fails outright, so that the plugin never tells the new window's page to put its tabs back, the page calls the app's `show_window` command (it takes a `main-` window label and only shows that window) so the tabs are not left in a window nothing shows. When the unit is every tab the window has, nothing moves and the window itself is dragged (the press window is the dragged window). The plugin shows the hidden window with GTK directly, finds its `xdg_toplevel` and starts the drag. The source page is told with `toplevel-drag-started`, stops its drag engine, and hears nothing more until `toplevel-drag-ended`. The window that holds the tabs when the drag ends acts on it: `dropped-on-window` merges the tabs into the target (end of its strip) and the emptied window closes; `cancelled` and `failed` move them back to their window and index; `dropped-elsewhere` leaves the window. The drag's payload (a JSON `TearPayload`) carries what moves and where it came from, and comes back on the end event, so the torn window's page can act even though only it can change its own session; the plugin also keeps the result until that page reads it, because a page that is still loading misses the event.

**Requirements and pitfalls found by running it** (headless Mutter 50.4 and a live GNOME Shell session, a standalone GTK3 window and a real Tauri window):

- GTK3 exposes the `wl_display` and each `wl_surface`, but not the `xdg_toplevel` or the implicit-grab serial. The toplevel and GTK's `wl_data_device` come from a C interposer on `wl_proxy_marshal_flags` that the final executable must export (`-rdynamic`, `-Wl,--undefined=wl_proxy_marshal_flags`, set in `apps/waypoint/src-tauri/build.rs`); the serial from a `wl_pointer` of our own on a separate queue of GTK's connection.
- Reuse GTK's own `wl_data_device`. A second device makes Mutter send drops to the wrong one.
- Dispatch the tracker's queue before reading the press serial: the page's own pointerdown is a few milliseconds old, and a periodic pump alone races it and yields serial 0.
- Show the window with GTK (`gtk_window().show_all()` on the main thread), not through Tauri's queued `show()`, before looking up its toplevel: GDK creates it on show.
- `cancelled` can arrive right after `dnd_finished`, so the end of a drag must be idempotent; destroy `xdg_toplevel_drag_v1` only after the drag has ended. A cancel after `dnd_drop_performed` means "released over nothing" (the window stays), a cancel without one is Esc (the window snaps back).
- After the drag starts the page gets no pointer events until the pointer re-enters (only `gotpointercapture` and `blur` come before), so the plugin tells it the drag started and ended and the page resets its drag engine.
- **GDK's implicit grab never ends.** The compositor takes the button release, so GDK keeps routing every pointer event to the window that was pressed: the dragged window and any window made afterwards get none, and a torn-off window can be seen but not used. The plugin ungrabs the pointer (`gdk_device_ungrab`) when the drag ends. This was found only by sweeping the pointer across a dropped window; a drop-only test passes without it.
- A WebKitGTK webview rejects a drop of a type it does not know unless it is added to the drop targets and `drag-motion`, `drag-drop` and `drag-data-received` accept it; wry's own handlers (only for the uri list) coexist when ours use another `info` value. The dragged window does not receive its own drag.
- The pointer capability can come and go (a remote desktop session ending, a pointer unplugged); the tracker follows it, or it holds a dead `wl_pointer` and every drag fails with "no pointer button is held".

**What was verified.** On headless Mutter 50.4 with screen capture frames: the new window follows the pointer at the grab offset and stays where it is dropped; Esc returns it (and the tab); a drop on another window merges the tab there and closes the torn window, for a tab, a pair and a whole window; the torn window takes pointer input afterwards. On a live GNOME session: `get_status` reports `toplevel_drag`, and a drag out of the strip ended `dropped-elsewhere` with the window left in place; the live screen then locked, so the rest was not repeated there. Not verified: a hardware pointer, KDE, fractional scale, a second monitor.

## Things Waypoint needs that Doska doesn't show

- **Merging into another window.** The pattern only tears off. To merge, each window needs to know where the other windows' tab rows are on screen. The backend can hold each window's frame plus tab-strip rect, hit-test the cursor on release, and emit a "receive tab" event to the target window.
- **Moving live state.** A torn-off tab carries its history, selection, split state and scroll position. Serialise it and hand it over through an event or a shared session store, then remove it from the source window.
- **Multi-monitor and scaling.** Positions from `screenX` and `screenY` are in logical pixels; the cursor position and window positions from Rust are physical. Convert with the monitor's scale factor.
- **Wayland.** Client-set window positions and always-on-top ghosts are restricted on some Wayland compositors. Test on GNOME (Mutter) and KDE (KWin). Fall back to opening the new window centred on the source and skip the floating ghost if positioning isn't allowed.
- **Windows 11 and X11.** Click-through ghosts work well on both, but test transparency on the ghost because it must respect the user's transparency setting.
- **Tab drags that also carry files.** If files are dragged, the ghost shows the file stack (see `interactions.md` §3.6) instead of a tab card.

## Pitfalls

- **`cursor_position()` lies on Wayland.** `tao` 0.37 returns `Ok((0, 0))` rather than an error (`tao/src/platform_impl/linux/util.rs:18`), so a ghost-follow thread would pin the ghost at the origin. The spike (`milestone-0-spikes.md`, Milestone 3 spikes) found it is also stale on X11 and XWayland unless a button is held and the pointer is over one of the app's windows, so `cursor_follow` is available only on X11 with a button held; report it as a feature flag, not a platform check.
- **`set_ignore_cursor_events` panics on a window that was never shown.** On Linux a GTK window that has not been realised panics (`tao/src/platform_impl/linux/event_loop.rs:457`; confirmed to abort on both Wayland and X11). Realise the ghost first (show it once, off-screen, on the main thread) and only then make it click-through.

- **`tauri-plugin-window-state` shows a hidden window.** It shows any window it has no saved state for, so a `visible(false)` ghost comes up visible, and it saves the ghost's state at exit. Denylist the ghost for as long as the plugin is registered.
- **A non-resizable ghost cannot be sized.** `resizable(false)` makes `set_size` a no-op on Linux. The ghost is resizable.
- **Show before positioning.** On X11 a position set before `show` is overridden at map time. On Wayland `set_position` is ignored.
- **Hit-test inner geometry, not `outer_position`.** For the frameless-shadow windows `outer_position` is 37 logical px above the client origin on X11, and on Wayland positions read back as (0, 0).
- **`is_always_on_top()` is a stored flag on Linux.** It is not evidence that the window is on top; on Wayland always-on-top does not hold once the source window is activated.
- **Native drag and drop is not the transport for X11 and Windows.** An OS-level drag stops in-page pointer events and cannot place the new window there, so the ghost-window design stays primary (A36). On Wayland the OS-level drag is exactly what moves a real window (`xdg-toplevel-drag`, above), and the page copes with the missing pointer events by being told when it starts and ends.

## Thresholds

One set is used everywhere (SPEC §13c, `interactions.md` §3.5 and §7): 4 px to start a drag, 24 px to tear off, a 450 ms hold to split, an 800 ms rest to start a group, and 140 ms of motion.

## Suggested build order

1. Ghost window with cursor follow and an end command, plus the timeout.
2. Tear-off from the tab strip, opening a new window at the drop point.
3. Session hand-off so the tab's state moves with it.
4. Merge-by-drop across windows.
5. Wayland: the real window through `xdg-toplevel-drag` where the compositor has it, and the in-page card with a compositor-placed window where it does not.
