# Milestone 0 spike results: listings and scans

Two of the five milestone 0 spikes (`README.md` §7) have run: **spike 1, the big-listing render**, and **spike 5, listing throughput**. This document records what was built, what was measured, what it means for the design, and what the spikes did not cover. The decisions that follow from it are A2, A9 and A18 in [`decisions.md`](decisions.md).

The spike code is throwaway and is not merged. It is kept on the `spike/listing` branch (commit `5fbc0d5`): a Rust module holding synthetic listings and scanning directories, a virtualised list, and a runner that logs every measurement as `SPIKE_RESULT` lines. It is built in release mode with `cargo build --release --features tauri/custom-protocol -p waypoint`, because the MCP bridge is debug-only and a debug build distorts the IPC numbers.

## Setup

| Item         | Value                                                                                                                               |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------- |
| Webview      | WebKitGTK (Safari 605.1.15 user agent) on GNOME under Wayland; the same build again with `GDK_BACKEND=x11` (XWayland)               |
| Window       | 1048 × 705 CSS pixels at a device pixel ratio of 2; `WEBKIT_DISABLE_DMABUF_RENDERER` unset                                          |
| List         | 500 000 rows (and 100 000), 28 px rows, pages of 256 rows, overscan 12, TanStack Virtual; selection kept as ranges                  |
| Transport    | Tauri commands returning JSON, and a raw packed-bytes response for comparison                                                       |
| Frame timing | `requestAnimationFrame` intervals while a script scrolls at a fixed rate; Wayland frames are paced at about 17 ms, XWayland at 7 ms |

## Spike 1: rendering a 500 000-row listing

| Measurement (Wayland)                                   | Result                                                                                                                                         |
| ------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| Wheel-speed scroll (32 rows per frame), 400 frames      | Frame time p50 17 ms, p95 18 ms, worst 18 to 21 ms; no frame over 33 ms; **no blank rows**                                                     |
| Extreme fling (714 rows per frame), 700 frames          | Frame time p50 17 ms, p95 18 ms; 0 to 5 frames over 20 ms; worst frame 31 ms in one run, 99 ms in another; every frame showed placeholder rows |
| Jump to a random row until visible rows have data       | 18 to 33 ms (one or two frames) across runs                                                                                                    |
| Select all, click, shift-click (next painted frame)     | 27 to 34 ms, the same for 100 000 and 500 000 rows                                                                                             |
| Sort 500 000 rows in Rust, then the visible rows update | Name 169 to 187 ms (case-insensitive byte order); size and modified 20 to 26 ms; visible 28 to 199 ms                                          |
| Range fetch round trip including the React update       | 2 to 3 ms at p50 for a 256-row page                                                                                                            |

Under XWayland the same suite ran at frame time p50 7 ms, p95 8 to 9 ms. Outliers were more visible there (worst frames 56 and 65 ms during the extreme fling, one or two frames over 33 ms per sweep), and jumps and select-all were correspondingly faster (8 ms and 14 ms), tracking the shorter frame interval.

**JSON against raw bytes.** Below 1 000 rows a range costs under 1 ms either way; at 5 000 rows JSON takes 2 to 3 ms and raw bytes 2 ms. Frame times, jumps and selection were indistinguishable. The typed JSON path is enough.

**The webview caps scrollable height.** A 2 000 000-row list (56 000 000 px) was clamped to **33 554 428 px**, which is 2²⁵ − 4. At 28 px rows that is about **1.198 million rows**. 500 000 rows (14 000 000 px) and 1 000 000 rows (28 000 000 px) are represented exactly.

## Spike 5: listing throughput

Fixture: 500 000 empty files in a directory on tmpfs, warm cache (created in 1.9 s). `read_dir` plus `DirEntry::metadata` for every entry.

| Measurement                                       | Result                                                                             |
| ------------------------------------------------- | ---------------------------------------------------------------------------------- |
| Baseline, no IPC                                  | 498 to 522 ms (about one million entries a second)                                 |
| Streamed over a `Channel`, batch of 500           | 592 to 626 ms total; first batch 0.6 ms in Rust, 2 to 15 ms in JS                  |
| Batch of 5 000                                    | 577 to 600 ms; first batch 5 ms in Rust, about 10 ms in JS                         |
| Batch of 50 000                                   | 573 to 591 ms; first batch 52 to 54 ms in Rust, 79 to 84 ms in JS                  |
| Batch of 5 000 with a live watcher and file churn | 563 to 573 ms; 8 001 watcher events for 2 000 created and removed files, none lost |

The `Channel` adds about 15 to 20 per cent over the bare scan. **When the command's promise resolved, JavaScript had received only 499 500, 495 000 and 450 000 of the 500 000 rows** for the three batch sizes: channel messages can still be in flight after the command returns.

## What this decides

- **React 19 with TanStack Virtual holds** for the file area on WebKitGTK at this scale: 60 Hz frames at wheel speed, instant jumps, near-instant selection. The SolidJS fallback is not triggered (A2).
- **Listing handles with range fetch work as designed** (A9): a 256-row page costs a few milliseconds, the JS heap holds only the pages near the viewport, and Rust does the sorting.
- **Findings the design must handle** are recorded as A18: the scroll-height cap, explicit end-of-stream messages, placeholder rows during a fling, a batch size of about 2 000 to 5 000, and typed JSON pages over raw bytes.

## Not covered

- **WebView2**, which has to be measured on Windows 11.
- **Native X11.** XWayland stands in for it here, and the two are not identical.
- **Cold caches, real disks and network filesystems.** The fixture was warm and on tmpfs; SFTP was not run.
- **A large viewport.** A maximised-window run hung before it logged anything and was abandoned, so only the 1048 × 705 window was measured; rendering cost is proportional to the visible rows, so it is expected to hold, but it is unmeasured.
- **Memory.** The spike process held about 400 MB resident with three 500 000-entry synthetic listings loaded; that is a coarse figure, not a per-listing cost.
- **The React Compiler**, natural and locale-aware sorting, icons and thumbnails, modifier-key selection beyond click and shift-click, and live watcher updates patching a visible list.

## Reproducing

Build the release binary on the `spike/listing` branch and run `target/release/waypoint` with the window's URL set to `index.html#spike-auto` (the branch's `tauri.conf.json` does this); read the `SPIKE_RESULT` lines from its output. Run with `GDK_BACKEND=x11` (and `XAUTHORITY` set for XWayland) for the X11 figures.

## Milestone 2 re-measurement

The spike measured throwaway code. Milestone 2 re-ran the same measurements against the real components: the real `waypoint-vfs` provider and plugin, the real list, a real 500 000-file folder on tmpfs made by `scripts/perf-fixture.sh`, in a release build with `VITE_WAYPOINT_PERF=1` so the dev-only harness in `apps/waypoint/src/dev/perfHarness.ts` (installed as `window.__waypointPerf`) is present. The window was 944 × 601 CSS pixels at a device pixel ratio of 2, on GNOME under Wayland and again with `GDK_BACKEND=x11` (XWayland). To repeat it, run `VITE_WAYPOINT_PERF=1 bun run --cwd apps/waypoint build`, a release build with `--features tauri/custom-protocol` and a window URL of `index.html#perf-auto=/tmp/waypoint-perf`, and read the `PERF_RESULT` line from the log; in a development build, open the folder and call `await __waypointPerf.runAll()` from the Tauri MCP bridge or the console.

| Budget (plan for milestone 2)                              | Wayland                                                                   | XWayland                                                                |
| ---------------------------------------------------------- | ------------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| No frame over 33 ms at wheel speed (32 rows a frame)       | Frame time p50 17 ms, p95 17 ms, worst 19 ms; no blank rows in 400 frames | Frame time p50 7 ms, p95 9 ms, worst 16 ms; no blank rows in 400 frames |
| No frame over 33 ms in an extreme fling (714 rows a frame) | p50 17 ms, worst 20 ms                                                    | p50 8 ms, worst 29 ms (two frames over 20 ms)                           |
| A jump to any row shows data within two frames             | 34 to 35 ms (two 17 ms frames)                                            | 16 to 21 ms (two 8 ms frames)                                           |
| Select-all paints within two frames                        | 33 ms                                                                     | 12 ms                                                                   |
| Sort by name within 200 ms in Rust                         | 43 to 68 ms (`waypoint-vfs` ignored benchmark, 500 000 entries, release)  | the same code                                                           |

**All budgets hold.** The real list matches the spike within noise: the same frame times, the same zero blank rows at wheel speed, and jumps and select-all one or two frames.

**Two honest caveats.** First, the harness's re-sort measurement (4 to 18 ms to a full set of rows) does not measure a sort: the list keeps the old rows on screen until the new page arrives, so no placeholder ever appears. The Rust figure above is the sort cost, and the time to repaint after one is not separately measured. Second, in a development build (debug Rust and the development frontend) 27 of 400 wheel-speed frames showed placeholder rows, where the release build showed none, so the budgets are only claimed for release builds.

**Not measured:** cold-cache scans (dropping the page cache needs root), WebView2 on Windows 11 (the VM run only checked that the app builds and lists folders), and the Folders tree at scale.

#### The grid, a maximised window and memory

Re-run later with `#perf-auto=/tmp/waypoint-perf&max&grid` (maximise first, and measure the grid as well), on GNOME under Wayland, the same 500 000 files, release build.

| Budget                                     | 944 × 601 window at 2×              | Maximised, 2560 × 1392 at 2×                              |
| ------------------------------------------ | ----------------------------------- | --------------------------------------------------------- |
| List at wheel speed (32 rows a frame)      | p50 14 ms, worst 14; no blank rows  | p50 17 ms, worst 18; no blank rows                        |
| Grid at wheel speed                        | p50 14 ms, worst 14; no blank rows  | p50 17 ms, worst 25; no blank rows                        |
| Fling (714 rows a frame), list and grid    | worst 15 ms (list) and 16 ms (grid) | worst 41 ms (list, one frame) and 34 ms (grid, one frame) |
| Jump to a row shows data within two frames | p50 23 ms                           | p50 34 ms (two 17 ms frames)                              |
| Select-all paints within two frames        | 20 ms (list and grid)               | 31 ms (list) and 30 ms (grid)                             |

**All budgets hold in both sizes.** Two things to know. First, `WEBKIT_DISABLE_DMABUF_RENDERER=1` (which `bun run tauri:dev` sets so the Web Inspector renders on Wayland) forces software compositing: maximised, it took every frame over 33 ms (p50 38 ms at wheel speed) where the default took 17 ms. The budgets are for a normal launch; a development build on a big window feels slower than the product will. Second, one early run at the small size showed a single 84 ms frame and a grid wheel sweep whose scroller had been replaced; the harness now reports `scrollerReplaced`, and two further runs showed neither, so it is recorded as seen once and not reproduced.

**Memory** (resident set, release build, sampled while the harness ran): with a 200-file folder open the app's Rust process held 263 MB and the WebKit web process 333 MB; with the 500 000-entry listing open and scrolled the Rust process held 346 to 397 MB (about 80 to 130 MB more, matching the plan's coarse 130 MB figure) and the web process 480 to 860 MB (330 to 530 MB more, dominated by the rendered surface and the cached pages, and larger when maximised). Closing a listing and the tab eviction were not measured separately.

### Windows 11 and accessibility passes

**Windows 11** (a VirtualBox VM, WebView2 154): the stack tip builds and runs. It lists `C:\Users\User`, shows the `C:\ > Users > User` breadcrumbs, 64 GB free, the Known Folders in Places, the empty Favourites state (there is no bookmarks file on Windows) and the Places / Folders switch. The pass found one real defect that Linux could not: `folderTree.ts` beside `FolderTree.tsx` resolves to the wrong file on a case-insensitive file system and fails `tsc`, so the module was renamed `treeRows.ts`. A second pass drove real mouse button events (the VM pins its pointer at the screen centre, so a script moves the window under it): clicking a place navigates, an empty folder shows "This folder is empty.", the mouse back and forward side buttons move through history, a middle-click on a folder opens it in a background tab without switching, and the grid switcher with its size slider works. Drag reordering (the pointer cannot be moved) and the Folders view on other drives are unchecked.

**Keyboard and roles**, audited in the running Linux app: the landmarks are a `main`, a `nav` for the location bar, a `nav` for the sidebar and polite status regions; the file list is a `listbox` named Files with `aria-multiselectable` and `aria-rowcount`; every button, tab and option has an accessible name; and the tab order follows `docs/accessibility.md` item 6 (window buttons, tab strip, toolbar, sidebar, files, status bar). Every Places and Favourites item is its own tab stop rather than a roving one, which suits a short list of buttons but is an open question for a screen-reader pass. The AT-SPI tree, which is what Orca reads, was dumped from the running app: a `Sidebar` landmark, a `Sidebar view` tab list, a `Tabs` tab list, a `Navigation` toolbar, the file list as a multi-select list box whose items are named like "name, size, date, kind" with their selected state, and the Folders tree with `level`, `posinset`, `setsize` and expanded state on every item. **Not done:** listening to Orca itself.

**Checks on the running Linux app** (development build): files created, renamed and deleted from a shell appear in an open folder within the second; Favourites against the real `~/.config/gtk-3.0/bookmarks` (Ctrl+D appends one line and leaves the others alone, F2 rewrites the label, Alt+Up reorders among the shown favourites without disturbing the remote bookmarks the sidebar does not show, Remove restores the file byte for byte); and a double-click on a file launches the system default application with its path. The first of these found that renaming a selected file dropped its selection, fixed by the `moved` field of the `Changed` event (A33). Real-pointer drag reordering is unchecked.

## Milestone 3 spikes

Spikes 3 and 4 of `README.md` §7 never ran in milestone 0, and the multi-window cost is new. They ran first on throwaway branches (`spike/tearoff`, `spike/native-dnd`) and are written up here. A10, A35 and A36 stay Proposed until they report.

### Multi-window cost on WebKitGTK

Each extra `main-N` window was created from Rust with `main-1`'s options and the real `index.html` bundle, so every window ran the whole app including the session plugin and a listing. The machine is GNOME Wayland (Mutter), one 3840 × 2160 monitor at 1.5×; GTK3 and `tao` see scale 2.0. Numbers are from release builds. "Ms to first paint" runs from the create call to the second `requestAnimationFrame` after React rendered into `#root`. Memory is summed over the app and every WebKit process, as RSS and as PSS (PSS divides shared pages between processes, so it is the fairer per-window figure).

| Extra windows | Platform | Total RSS (MB) | Total PSS (MB) | Web processes | Ms to first paint (per new window, created one at a time) |
| ------------- | -------- | -------------- | -------------- | ------------- | --------------------------------------------------------- |
| 0             | Wayland  | 653 to 672     | 282 to 299     | 1             | n/a                                                       |
| 1             | Wayland  | 985 to 1018    | 384 to 414     | 2             | 158 to 179                                                |
| 2             | Wayland  | 1308 to 1354   | 473 to 516     | 3             | 156 to 172                                                |
| 4             | Wayland  | 1976 to 2039   | 664 to 727     | 5             | 155 to 170                                                |
| 8             | Wayland  | 3253 to 3395   | 981 to 1122    | 9             | 155 to 170                                                |
| 0             | XWayland | 637            | 285            | 1             | n/a                                                       |
| 1             | XWayland | 990            | 401            | 2             | 154                                                       |
| 2             | XWayland | 1325           | 501            | 3             | 149                                                       |
| 4             | XWayland | 1992           | 687            | 5             | 154 to 168                                                |
| 8             | XWayland | 3301           | 1023           | 9             | 148 to 183                                                |

- **One web process per window**, about 310 to 330 MB RSS each, which is roughly 330 MB RSS or 80 to 130 MB PSS per added window. The Rust process grows about 7 MB per window and the WebKit network process stays near 59 MB.
- **Creation is cheap and paint is fast.** `WebviewWindowBuilder::build` returns in 27 to 43 ms and a window paints in about 160 ms. Creating 8 windows in one burst, all 8 painted within about 520 ms on Wayland (690 to 940 ms with the machine under load from other work) and 470 to 650 ms on XWayland. A cold first run, under load, took about 620 to 680 ms per window.
- **The ceiling in the plan holds on time but is ambiguous on memory.** About 600 ms is not reached in normal conditions. About 150 MB per window is exceeded by RSS (about 320 MB) and met by PSS (80 to 130 MB). Total memory is the real limit: 8 extra windows cost about 2.6 GB RSS, or about 0.8 GB PSS. **Recommendation:** warn at 8 windows and cap at 12, and re-measure PSS on a low-memory machine before the cap is final.
- **Rendering.** `main-2` and `main-3` showed the themed UI with rounded frameless corners and a loaded listing, and `html` and `body` were `rgba(0, 0, 0, 0)` at first paint in every spike window, so there is no white background. That does not prove the absence of a one-frame flash: GNOME Shell denies screenshots and the bridge captures only the webview, and only the dev build (software compositing) was looked at.
- **Not measured:** idle CPU per window, 12 to 16 windows, and anything on Windows or KDE.

### Tear-off ghost and hit-testing

Run on GNOME Wayland and under XWayland (`GDK_BACKEND=x11`), with the real pointer moved through the Mutter RemoteDesktop D-Bus interface (motion and left-button down and up only). Marks: Confirmed (read in source or reproduced), Refuted, or Untested.

**The ghost window**

- **Unrealised-ghost panic: Confirmed on both backends.** `set_ignore_cursor_events(true)` on a window that was never shown aborts: `tao` `event_loop.rs:457` calls `window.window().unwrap()`, the panic is a non-unwinding abort (exit 134) and the call had already returned `Ok(())`.
- **A trap the plan missed: `tauri-plugin-window-state` shows a `visible(false)` window.** It defaults to showing any window it has no saved state for, so a hidden ghost came up visible, `is_visible()` returned true and no panic occurred. The panic reproduced only after the ghost was denylisted, and the plugin also saves the ghost's state at exit. The ghost must be denylisted for as long as the plugin is registered (slice 03 removes the plugin; the denylist matters until then).
- **Realise first: Confirmed.** `set_position(-5000, -5000); show(); set_ignore_cursor_events(true); hide()` works on Wayland and X11 with no sleeps, because requests run in order on the GTK event loop. On Wayland `set_position` is a no-op, so the ghost may flash briefly where the compositor puts it.
- **`resizable(false)` breaks sizing: Confirmed on both.** A non-resizable ghost stayed at 400 × 400 physical and ignored `set_size(160, 100)`. With `resizable(true)` the size is honoured. The ghost must be resizable.
- **Order of `show` and `set_position`.** On X11 a position set before `show` is overridden at map time (read back as (0, 0)); after `show` it works and `xdotool` agrees. On Wayland `show` works and `set_position` is silently ignored (the compositor centred the ghost). So show first, then position, then keep following.
- **Always-on-top.** X11: Confirmed (`_NET_WM_STATE_ABOVE`, `SKIP_PAGER` and `SKIP_TASKBAR` per `xprop`). Wayland: no usable signal. `is_always_on_top()` returned false after building with `always_on_top(true)`, and on GNOME the ghost sits above the source window only until that window is clicked or activated (a source window that is not re-activated during a drag probably holds, which is unproven). `is_always_on_top()` is a stored flag in `tao` on Linux, so it is not evidence on either backend; use `xprop` on X11 and nothing on Wayland.
- **Pointer capture and focus.** A click-through ghost under the pointer lost nothing on X11 or Wayland: the source's `pointermove` stream was continuous through the ghost region, a click at the ghost centre reached the source, and a button-held drag with `setPointerCapture` carried on across it. On Wayland the ghost never took focus (one run; not repeated). A solid ghost (not click-through) is unusable: on X11 pointer events stopped inside its region and a click at its centre never reached the source; on Wayland it took keyboard focus and the source got `blur` then `focus` even with `focusable(false)`. So click-through before `show` is required.

**Cursor position**

- **Wayland: Confirmed.** `tao` 0.37.1 `linux/util.rs:18` returns `Ok((0, 0))` on a Wayland display. `app.cursor_position()` and `window.cursor_position()` returned exactly (0, 0) over 931 polls at 16 ms, with the pointer injected to five places and during a button-held drag. It is a lying success, so `cursor_follow` has to be a feature flag.
- **XWayland: live only under some conditions.** With no button held, the value follows the pointer only while it is over one of the app's own X windows, and stays stale at the last in-window value after it leaves, with no error. With the left button held, which is the tab-drag case, it was live everywhere, outside every window and across the whole screen, at 1 to 2 ms per poll, and it updates to the release point. The value is in `tao`'s physical space. **So `cursor_follow` is available only on X11 with a button held.**
- **Windows: source read, not run.** `cursor_position` is `GetCursorPos`, global and live.

**Hit-testing across windows**

- **Wayland: Refuted.** `outer_position` and `inner_position` return (0, 0) for every window, including after `set_position`, which is a silent no-op. `outer_size` is real. A backend hit-test against other windows' frames cannot work on Wayland.
- **X11 and XWayland: Confirmed, with a caveat.** `set_position` moves the window and reads back correctly (checked with `xdotool`). For the frameless-shadow `main-N` windows it sets the inner (client) origin, and `outer_position` is 74 physical px (37 logical) above it, so a hit-test must use `inner_position` plus `inner_size`, or the page's tab-strip rectangle, not `outer_position`. `window.screenX/Y` in the page follows the outer position.
- **Cross-monitor: untested** (`tao` listed a second monitor at x = 5120 that Mutter does not report).

**Logical and physical**

- **X11: Confirmed.** `target_physical = (screen_logical - grab_logical) × scale`, with `scale` the window's own `scale_factor()` (2.0 here, not Mutter's 1.5) and the shadow inset added for `main-N` windows. A pointer at logical (1000, 500) with a grab offset of (120, 14) put the window at inner (1760, 972) and `screenX` 880. Pointer events' `screenX/Y` are logical and track the pointer on both backends.
- **Wayland: Refuted for placement.** The position can be computed but not applied: a new window ignores it and the page reads `screenX/Y = (0, 0)`.
- GTK3 reports scale 2 while Mutter's real scale is 1.5, so physical pixels in `tao` are virtual. The formula is internally consistent but not real device pixels.

**Wayland conclusion.** There is no cursor, no window positions, no window frames, no ghost positioning and no reliable always-on-top, so `cursor_follow`, `window_position` and `hit_test` are all false there, as the plan says (D94). A static click-through ghost is feasible but would sit centred and not follow the pointer, so it is not worth showing. The degraded path is an in-page preview, a compositor-placed new window, and merge by menu.

**Windows (source read, not run).** `set_ignore_cursor_events` sets `WS_EX_TRANSPARENT | WS_EX_LAYERED` and is not on the Linux realise path, so the panic should not apply (unverified). `focusable(false)` maps to `WS_EX_NOACTIVATE` in `tao`, so that concern may already be handled. `show` uses `SW_SHOWNOACTIVATE` where `tao` does.

**Changes the spike makes to the design (A42):** the ghost is resizable; the ghost is denylisted in `tauri-plugin-window-state`; show before position; hit-test on inner geometry and registered regions, never `outer_position`; `cursor_follow` is available only on X11 with a button held, and `window_position` and `hit_test` are probed at runtime (set a position and read it back) rather than assumed from the platform; `is_always_on_top()` is never read as evidence.

### Native drag and drop and `dragDropEnabled`

Read from the sources of `tauri` 2.12.0, `tauri-runtime` and `tauri-runtime-wry` 2.12.0, `tauri-utils` 2.9.3, `wry` 0.57.0 and `tao` 0.37.1 (the versions in `Cargo.lock`), plus a scripted run in two spike windows on this machine. Marks as above, with Inferred where something follows from source but was not run.

- **Naming (Confirmed).** There is no cross-platform `drag_and_drop(bool)` on the window builder that controls file-drop events. `dragDropEnabled` in the config maps to `disable_drag_drop_handler()` (`tauri-2.12.0/src/webview/webview_window.rs:1098`), which decides whether wry's drag-drop handler is installed (`tauri-runtime-wry-2.12.0/src/lib.rs:4799`). `drag_and_drop(bool)` exists only on Windows (`webview_window.rs:781`) and is `tao`'s switch for a drop target on the top-level window, a different HWND from the one wry registers, and not what the HTML5 conflict is about.
- **Events (Confirmed).** `tauri://drag-enter`, `drag-over`, `drag-drop` and `drag-leave`, emitted only to the window whose webview received the drag, with `{ paths?, position }` (`paths` is absent on `over`; `leave` has no payload). A dropped path is added to the fs scope as a side effect (`manager/window.rs:244-252`). Because each window has its own handler, per-window targeting needs no routing.
- **Fixed at creation (Confirmed).** No setter exists anywhere in `tauri`, `tauri-runtime`, `tauri-runtime-wry` or `wry`; the handler is installed when the webview is built (GTK `connect_drag_*`, Windows `RegisterDragDrop`). Changing the policy means recreating the window.
- **Linux (Confirmed in code, Inferred for WebKit's target ids).** wry connects `drag_data_received`, `drag_motion`, `drag_drop` and `drag_leave`; `drag_motion` always returns false and `drag_data_received` only emits, so WebKit's own drag handling still runs and HTML5 drag events keep working with the handler on. A drop carrying `text/uri-list` is finished by the handler, so a file drop never reaches the page. Only file lists produce events.
- **Windows (Confirmed).** wry calls `SetAllowExternalDrop(false)` and registers its own `IDropTarget` on the child HWNDs, which is the documented conflict: HTML5 `dragover` and `drop` never fire in the page on Windows, for any drag. macOS blocks HTML5 file drops the same way.
- **The conflict is with HTML5, not with pointer drags (Confirmed in source, Inferred at runtime).** Pointer events travel through normal mouse input and never reach a registered drop target, so the tab strip's pointer-capture reorder is independent of the handler on every platform. A scripted pointer sequence with capture logged identical events in a window with the handler on and one with it off. That is DOM-level only: it cannot show anything about native input routing or an OS-level drag in progress. The plan's worry that `true` blocks pointer drags on Windows is not supported; the real cost of `true` there is that HTML5 drag and drop dies, which Waypoint does not use.
- **Positions (Confirmed in code, not run).** Windows reports client-relative physical pixels. On Linux the GTK values go straight into `PhysicalPosition` with no scale applied, and GTK3 widget coordinates are logical, so on a scaled Linux display the "physical" value is probably logical. Code that consumes drop positions must not multiply by `scale_factor` blindly on Linux.
- **Wayland (Inferred).** GTK3 uses the native `wl_data_device` unless `GDK_BACKEND=x11`, so drops are native Wayland, not XDND under XWayland. Motion is reported only while the drag is over the surface, so there is no screen position. Drops from sandboxed apps may arrive as portal paths (`/run/user/.../doc/...`).
- **Outbound drags (Confirmed absent in core).** Tauri has nothing for starting a drag. On GTK a plugin needs `gtk_drag_begin_with_coordinates` with a target list of `text/uri-list` and `x-special/gnome-copied-files`, a `drag-data-get` handler and the pointer-press serial that Wayland requires. On Windows it needs `DoDragDrop` with an `IDataObject` carrying `CF_HDROP` and an `IDropSource`, on the main thread, and a drag that ends over the plugin's own window hits wry's drop target, so a self-drop emits `tauri://drag-drop` and must be filtered. A plugin cannot register its own drop target on the webview HWNDs without revoking wry's. Existing crates were not in the registry (`drag-rs` and a plugin wrapper, `tauri-plugin-drag`, are candidates; check licence and Wayland support, and the serial handling in the Linux source, before writing one).
- **Native drag and drop as the tear-off transport: not recommended.** The OS draws the icon, Esc cancels, and Windows delivers a drop to another window's drop target. But once an OS drag starts, in-page pointer events stop, so the strip's own animation and hit-testing stop; a custom data format conflicts with wry's `CF_HDROP`-only target on Windows; on Wayland the new window still cannot be placed at the drop point; and it would couple the tab strip to a native plugin. The ghost-window design stays primary.
- **Decision.** Windows Waypoint creates at runtime in milestone 3 disable the handler (`disable_drag_drop_handler()`, config `dragDropEnabled: false`) on every platform. On Windows that keeps HTML5 drag and drop available for any future web content, and costs nothing now because nothing consumes the events; on Linux and macOS it avoids silently swallowing file drops that nothing handles. Windows that must accept file drops in milestone 4 are decided with `native-dnd`'s design, which also settles who owns the inbound path (wry's handler or the plugin). `native-dnd` is built in milestone 4, not now, because it has no consumer in milestone 3 and a skeleton would add unsafe GTK and OLE code to `windows-check` for nothing.

### Wayland tear-off with `xdg-toplevel-drag`

The spike on `spike/toplevel-drag` (a real GTK toplevel attached to a `wl_data_device` drag through `xdg_toplevel_drag_v1`, on headless Mutter 50.4 and a live GNOME Shell, standalone GTK3 and a Tauri window) became slice 13. It was then run in the app: on headless Mutter the new window followed the pointer and stayed where it was dropped, Esc returned it, and a drop on another window merged the tabs and closed the torn window; on the live session `get_status` reported `toplevel_drag` and one drag ended `dropped-elsewhere`. Running it also found that GDK's implicit pointer grab never ends, leaving a dropped window without input, and that the pointer capability can come and go. The design, the requirements and what was not verified are in [`docs/tauri-tear-off.md`](../tauri-tear-off.md).

### What the owner must test by hand

Linux, GNOME Wayland:

1. A real tab drag (not injected input): the ghost stays above the source window for the whole drag and the source window gets no `blur`; repeat the click-through ghost show 10 times and confirm no `blur` ever reaches the source.
2. Drag a file from Nautilus onto a window with the handler on and one with it off (the `spike/native-dnd` branch has a `WAYPOINT_DND_SPIKE=1` harness): events only in the "on" window, a position that is probably logical; a drop from a Flatpak app; the HTML5 `dragstart` and `drop` of an element inside the "on" window still fire; a real pointer drag with capture behaves the same in both.
3. Drag a file out with `drag-rs` to Nautilus and a terminal, with and without the pointer-press serial.

XWayland and X11:

4. Drag a tab out and release outside every Waypoint window: `cursor_position` stays live until release at 16 ms; no flicker at the ghost's edge at normal speed.
5. Two monitors, ideally with different scale factors: the placement formula and the inset land correctly on the second one.

KDE/KWin Wayland:

6. Repeat positions, always-on-top and `cursor_position`, since this design assumes GNOME's behaviour is representative.

Windows 11 VM (the spike harnesses are `WP_SPIKE` scenarios on `spike/tearoff` and the `WAYPOINT_DND_SPIKE` harness on `spike/native-dnd`):

7. Ghost realise then click-through; the ghost does not steal activation; always-on-top holds over other windows; `cursor_position` is live over every window and outside them at 16 ms; the 1, 2, 4 and 8 window cost on WebView2; a mixed-scale monitor pair.
8. HTML5 drag events fire only with the handler off; an Explorer file drop gives `drag-enter`, `over`, `drop` and `leave` with physical positions at 100%, 150% and 200% scaling and on a second monitor; drops go to the window under the cursor and only to its label; two windows created in quick succession both accept drops; a tab-strip pointer reorder with capture is identical with the handler on and off, for mouse, touch and pen; a drag out with `DoDragDrop` onto Explorer, a terminal and the same window.

Release-build rendering on real hardware (no `WEBKIT_DISABLE_DMABUF_RENDERER`):

9. Per-window CPU while idle and while a ghost follows the pointer, and transparent corners on windows 2 to 8.

### Not measured

Per-window idle CPU and a compositor-level flash check; always-on-top on Wayland during a real drag; cross-monitor and mixed-scale behaviour; KDE; anything on Windows 11; a real ghost following a real tab drag from end to end; the real receive path for file drops.

### Milestone 3 verification

Run on the tip of the milestone 3 stack (13 branches), release build (`bun run --cwd apps/waypoint build` and `cargo build -p waypoint --release --features tauri/custom-protocol`) without `WEBKIT_DISABLE_DMABUF_RENDERER`. The owner's GNOME session was locked, so the passes ran in a private headless Mutter (`mutter --headless --wayland --virtual-monitor 1280x720`, its own D-Bus session and its own `XDG_DATA_HOME`, `XDG_CONFIG_HOME` and `GSETTINGS_BACKEND=keyfile`), with the pointer moved through that Mutter's RemoteDesktop interface (relative motion and left button only). XWayland numbers are from that Mutter's XWayland (`GDK_BACKEND=x11`), not a native X server. The timings come from a throwaway page script (not committed) that logged `Date.now()` stamps to a local HTTP server, and `/proc/<pid>/smaps_rollup` and `/proc/<pid>/stat` sampled for the app and every descendant process.

**New window to first listing paint.** From the page asking for a window (a synthetic `Ctrl+Shift+N`) to the new page's first animation frame after its first listing row exists, seven windows opened one after another, three seconds apart: **155 to 169 ms** (the new page's script started 101 to 110 ms after the request). The budget is 600 ms; the milestone 3 spike saw about 160 ms.

**Memory per window** (RSS and PSS summed over the Rust process and every WebKit process; one hidden `tear-ghost` window is always present and is in every row):

| Main windows | Processes | RSS (MB) | PSS (MB) | Rust process PSS (MB) |
| ------------ | --------- | -------- | -------- | --------------------- |
| 1            | 4         | 755      | 373      | 99                    |
| 2            | 5         | 1037     | 476      | 107                   |
| 4            | 7         | 1586     | 662      | 113                   |
| 8            | 11        | 2615     | 957      | 128                   |

That is about **266 MB RSS and 83 MB PSS per added window** (the spike measured about 330 MB RSS and 80 to 130 MB PSS), with the Rust process growing about 4 MB per window. Each window is one more web process; the network process is shared.

**The source window's frame times during a tab drag** (`requestAnimationFrame` intervals in the dragged-from window, 60 Hz virtual monitor, maximised):

| Drag                                                                                                                                      | Frames | p50   | p95   | p99   | Worst | Over 33 ms |
| ----------------------------------------------------------------------------------------------------------------------------------------- | ------ | ----- | ----- | ----- | ----- | ---------- |
| Reorder back and forth along a strip of five tabs (about 11 s of a 16 s sample)                                                           | 997    | 16 ms | 17 ms | 18 ms | 22 ms | 0          |
| Pull a tab out of the strip and move the pointer around (12 s; the release build logs too little to confirm the compositor took the drag) | 747    | 16 ms | 17 ms | 17 ms | 30 ms | 0          |

**The ghost's CPU on X11 (XWayland) over a 10 s held drag**, as a share of one core: idle, all processes together 0.5 %. During the held drag the Rust process (the follow thread) used 3.2 %, against 0.8 % in a control drag that stayed inside the strip, and the ghost's web process 1.0 % against 0.1 %; the page being dragged from used 4.7 % against 3.4 %. The ghost's share is therefore about **3.3 %** (budget 5 %); all processes together were 9.0 % against 4.4 %.

**The 500 000-file budgets with split panes open** (`scripts/perf-fixture.sh /tmp/waypoint-perf 500000`, the `#perf-auto=…&max&grid` harness, release build, Wayland, maximised 1280 by 720, a pair with the same folder open in both panes, so two 500 000-entry listings live at once):

| Measure                                      | List                                                          | Grid                                                          |
| -------------------------------------------- | ------------------------------------------------------------- | ------------------------------------------------------------- |
| Wheel speed (32 rows a frame), 400 frames    | p50 17 ms, worst 18 ms, no blank rows                         | p50 17 ms, worst 21 ms, no blank rows                         |
| Fling (714 rows a frame), 700 and 640 frames | p50 17 ms, worst 22 ms, one frame over 20 ms, none over 33 ms | p50 17 ms, worst 21 ms, one frame over 20 ms, none over 33 ms |
| Jump to a row shows data                     | p50 34 ms (two frames), p95 35 ms                             | not measured by the harness                                   |
| Select all paints                            | 32 ms                                                         | 31 ms                                                         |
| Sort (visible rows updated)                  | 16 to 17 ms                                                   | not measured by the harness                                   |

Peak memory during that run was 1168 MB RSS and 787 MB PSS (Rust process 298 MB PSS, WebKit 499 MB PSS). All budgets hold. The harness looks at the first list on screen and opens its folder only in the active tab, so for this run it was changed (not committed) to open the folder in every pane's tab.

**Windows 11** (the VM pass, reported separately): about 95 MB per window (WebView2 working set about 82 MB plus about 11 MB for the app) and 184 to 232 ms per `open_window`; a nine-window session restored, and the 12-window cap held.

**Geometry across restarts on X11 (XWayland).** Two windows placed at (100, 80) 640 by 400 and (520, 240) 640 by 420 came back at the same inner position and size after each of three restarts, and the saved geometry was unchanged; a window opened with `Ctrl+Shift+N` landed 30 px down and to the right of the one it came from. On Wayland the compositor places windows, so only sizes are restored.

**What this does not cover.**

- It is a headless compositor with a virtual monitor, not the owner's GPU and monitors; frame times are paced at the virtual monitor's 60 Hz.
- RemoteDesktop keyboard events did not reach the webview in the headless session (no keyboard focus), so Escape during an in-page drag was driven with in-page key events; Escape did cancel a compositor-held window drag.
- Under GNOME's Mutter a cancelled `xdg-toplevel-drag` logged a critical assertion (`meta_dnd_actor_drag_finish`) and, once, the headless Mutter aborted about 13 s later in its cursor-theme code; it did not recur in later runs and is a compositor-side fault.
- KDE, native X11 with server-side decorations and macOS were not run.

## Milestone 4: outbound drag and the file clipboard

_Results to be added from the slice 00 spike._
