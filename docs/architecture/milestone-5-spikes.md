# Milestone 5 spike results: window transparency and blur

This records the transparency spike (#166) that milestone 0 never ran, as far as it could be run for slice 6 (#145). It says what was measured, on what, and what remains unverified. Nothing here is measured on GNOME Wayland's real compositor, X11, KDE or Windows.

## Setup

| Item         | Value                                                                                                                                                                                                                      |
| ------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Compositor   | Weston 15.0.1 (headless backend, pixman renderer, 1280 × 800) in a private D-Bus session, with a striped wallpaper of yellow, blue, white, black, pink and green bands behind the app. The desktop session was not touched |
| App          | `bun run tauri:dev` debug build, WebKitGTK 2.x over GDK's Wayland backend, `WEBKIT_DISABLE_DMABUF_RENDERER=1` (set by the script), `XDG_CURRENT_DESKTOP=GNOME` so the plugin reports GNOME's reasons                       |
| Screenshots  | `weston-screenshooter`, which captures the composited output, so what is checked is what the compositor drew, not the webview's own render                                                                                 |
| Frame timing | `requestAnimationFrame` intervals in the page, with a magenta box animated every frame for 4 s                                                                                                                             |

Weston is not Mutter: it composes software buffers on the CPU and has no blur protocol, no tiling and no window focus without an input device. It shows what an alpha page does to a compositor that honours premultiplied alpha, which every Wayland compositor does. It says nothing about GPU compositing cost.

## What the plugin reported

On the setup above `get_status` gave `opacity` and `shadowInset` available, and `blur` unavailable with `compositor-has-no-blur` ("GNOME does not let apps blur behind their windows"); Mica and Acrylic belong to Windows. No blur protocol was available to try.

## Does a translucent page show the desktop through a WebKitGTK window?

**Yes, on this setup.** Every Waypoint window is created transparent (`.transparent(true)`, frameless; A40 and D89), whatever the setting, so no creation-time choice arises and nothing needs a restart. With the master switch on, the page's backgrounds took alpha and the wallpaper's yellow title band, green sidebar band and blue and pink content bands showed through the title bar, the tab and toolbar rows and the sidebar, and a Settings window laid over the main window showed the main window's text through its navigation. With the switch off the same windows were solid. High contrast, reduced transparency, an unavailable platform and "not in front" each gave a solid window (covered by unit tests; "not in front" was also seen live, because Weston never gives a window focus).

The default opacities (82 % on the title bar, rows +8, sidebar +12, file area +16, so 90, 94 and 98 %) read as a tinted pane, not as glass: the file area is deliberately nearly solid. Lower values show the wallpaper's bands plainly.

## Cost

| Measurement                                                      | Transparency off                      | Transparency on                       |
| ---------------------------------------------------------------- | ------------------------------------- | ------------------------------------- |
| Animated box for 4 s (frames, p50, p95, worst)                   | 250 frames, 16 ms, 16 ms, 17 ms       | 250 frames, 16 ms, 16 ms, 17 ms       |
| Idle CPU of the app and its WebKit processes over 10 s           | not taken                             | 0 % of a core                         |
| Two programmatic resizes with the page recording frame intervals | 165 intervals, p50 16 ms, worst 23 ms | 148 intervals, p50 16 ms, worst 19 ms |

These show no extra cost here, but the setup cannot show it: pixman composition on the CPU of a headless compositor, DMABUF off, a single animated box, and two resize steps rather than a continuous drag. **No "solid while resizing" mode was built**, because nothing dropped; the question stays open for a GPU-composited GNOME session and for Windows Acrylic, which the plugin already flags as able to lag while a window is resized.

## Does the shadow inset fix the half-tiled margin?

Not established. `set_shadow_inset` is called for every framed window after its page starts loading and after each resize, with the frame's 8 px margin (zero while maximised), and it returned no error. Weston cannot tile a window, so the half-tiled case (the artefact `gdk_window_set_shadow_width` is meant to fix) was not seen either way. A window maximised under it sat flush against the output's edges with no margin. `docs/theming-and-platforms.md` keeps the known limitation until a GNOME or KDE session has tiled a window.

## Contrast floor

For the light and dark themes' text on each surface the floor (the least opacity that keeps 4.5:1 over the worst backdrop, blended in sRGB) comes to about 52 % (light, over black) and 41 % (dark, over a light grey `#a0a0a0`). A pure white wallpaper behind a dark translucent window can still fall below 4.5:1; only "Solid when unfocused" and blur lessen that. The numbers are from the pure function `alphaFloor` and its tests, not from a measurement of rendered pixels.

## Not verified

- **GNOME Wayland (Mutter), real GPU:** that the DMABUF renderer draws the alpha page, the resize and idle cost with it on, and the half-tiled margin. Mutter has no blur API, so blur stays hidden there.
- **X11 and XWayland:** the screen's RGBA visual, `_KDE_NET_WM_BLUR_BEHIND_REGION` and the shadow inset.
- **KDE Plasma (Wayland and X11):** `ext_background_effect_manager_v1` and `org_kde_kwin_blur_manager` binding, the blur region (the visible window, inside the shadow margin, refitted 150 ms after a resize), and tiling.
- **Windows (the VM):** Mica and Acrylic through `set_effects`, whether they draw without GPU acceleration, Acrylic's lag while resizing, and the Low (Mica) and High (Acrylic) mapping. Slice 26 does these.
- **Focus changes:** that the page's `focus` and `blur` events and Rust's `Focused` event agree on a real compositor, so a window turns solid and back as it leaves and enters the front.
- **The Experimental label** stays on the Linux page until GNOME and KDE sessions have been looked at.

# Thumbnail delivery (spike #167)

This records the thumbnail delivery spike: can WebKitGTK show a 10,000-image folder through the custom `thumb://` scheme at 60 Hz with acceptable memory, and what does delivery cost. It says what was measured, on what, and what remains unverified. Nothing here is measured with a GPU, on X11, on Windows or on a spinning disk, and the app was a debug build.

## Setup

| Item         | Value                                                                                                                                                                                                                                                      |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Compositor   | Weston 15.0.1 (headless backend, pixman renderer, 1280 × 800) in a private D-Bus session, as for the transparency spike. The Waypoint window was 1100 × 720, so a screen held 45 tiles (11 columns) and 126 to 140 tiles were in the DOM with the overscan |
| Webview      | WebKitGTK 2.52.6 over GDK's Wayland backend, `WEBKIT_DISABLE_DMABUF_RENDERER=1` (set by `bun run tauri:dev`)                                                                                                                                               |
| App          | `bun run tauri:dev`, a debug (unoptimised) Rust build, with private `XDG_DATA_HOME`, `XDG_CONFIG_HOME` and `XDG_CACHE_HOME`, so the thumbnail cache started empty and the user's session was not touched                                                   |
| Fixture      | `scripts/thumb-fixture.sh`: 10,000 PNG and JPEG images (200 × 150 to 640 × 480, 183 MB) in `/tmp` (tmpfs on this host, so file reads were from memory). Their thumbnails are 5.8 KB each on average and 79 MB in all                                       |
| Timing       | `requestAnimationFrame` deltas in the page, `performance.now()` (1 ms resolution here), and `VmRSS` from `/proc` for the app and each `WebKitWebProcess`, every 500 ms                                                                                     |
| What is real | The grid, the loader, the bridge commands, the plugin queue, the cache on disk and the `thumb://` scheme are the shipped ones. The harness only reads the page and scrolls it                                                                              |

The harness is `scripts/thumb-delivery-harness.mjs` (not shipped, not imported by the app). It is loaded by a development build's webview through Vite's file server and driven from the Tauri MCP bridge's `webview_execute_js`.

## A defect the measurements found, and its fix

The first measurement of a jump to a screen of cached tiles was slow and bimodal: about 22 ms when the loader already held the pictures and about 240 ms when it had to ask. The plugin was not the cause: timed from inside `thumbnails_request_entries`, a 135-tile batch of cache hits was served and sent in under 2 ms. The time was in the bridge, which resolved each entry of the batch with its own `Vfs::resolve_selection`, and that builds a set of the ids and walks the whole view for each one: about 195 ms for 135 entries of a 10,000-entry folder in a debug build, growing with both the batch and the folder. `Vfs::locate_entries` now answers a batch with one lookup per id, and the bridge uses it (commit `fix(thumbnails): locate a batch of entries without walking the listing for each`, with a test).

The difference was measured on fresh app processes, same protocol, with the fix reverted and applied:

| Measurement (60 random jumps, 45 tiles in view, cached) | Before the fix          | After the fix        |
| ------------------------------------------------------- | ----------------------- | -------------------- |
| First picture, p50 / p95                                | 51 / 322 ms             | 38 / 45 ms           |
| Every tile in view painted, p50 / p95 / worst           | 196 / 322 / 353 ms      | 42 / 50 / 51 ms      |
| Jumps over the 150 ms budget                            | 30 of 60                | 0 of 60              |
| Stepping a screen at a time down the folder, p50 / p95  | 31 to 35 / 54 to 182 ms | 4 to 5 / 16 to 19 ms |

Release builds are faster at this, so the size of the defect there is smaller, but its shape (cost per entry proportional to the folder) is the same.

## Results (after the fix)

| What                                                                                                              | Result                                                                                                                                                                                                                                  | Budget                                                     |
| ----------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------- |
| `thumb://` serve time, cache hit, 1,000 requests one at a time (`fetch`, `no-store`)                              | p50 0 ms, p95 1 ms, worst 1 ms (timer resolution is 1 ms); mean body 5,783 bytes; none failed                                                                                                                                           | 5 ms or less at p95: met                                   |
| The same, 60 at once, 10 rounds                                                                                   | each request p50 2 ms, p95 4 ms; the 60 done in 3 ms (p50) and 5 ms (p95)                                                                                                                                                               | met                                                        |
| A screen of cached tiles after a jump (see above)                                                                 | painted p50 42 ms, p95 50 ms, worst 63 ms over 120 jumps in two runs, none over 150 ms                                                                                                                                                  | 150 ms: met                                                |
| Frame times, 60 px per frame (3,600 px/s, about 23 rows/s), whole folder                                          | p50 16, p95 17, worst 24 ms; 18 of 2,364 frames over 20 ms; none over 33 ms                                                                                                                                                             | 60 Hz held, no task over 50 ms: met                        |
| 200 px per frame, three more runs                                                                                 | p95 21 to 22 ms, worst 32 to 42 ms, none over 50 ms (one earlier run had five frames of 51 to 67 ms in its first touch)                                                                                                                 | not met in every run                                       |
| 900 px per frame (a fling), three runs                                                                            | p50 29 ms, p95 33 to 34 ms, worst 36 to 43 ms, none over 50 ms (one earlier run had a 303 ms frame)                                                                                                                                     | 60 Hz not held while flinging                              |
| Same sweeps over 10,000 empty `.dat` files in the grid (no thumbnails)                                            | 60 px: p50 16, p95 17, worst 25 ms. 200 px: p95 17, worst 21 ms. 900 px: p50 16, p95 19, worst 22 ms                                                                                                                                    | the baseline                                               |
| Webview memory over four full scrolls of the folder                                                               | 300 MB when idle on an empty window, 660 to 700 MB idle after each scroll, peaks of 950 to 1,030 MB, no growth beyond the fourth scroll (a longer earlier run, 11 scrolls, held 565 to 570 MB idle and peaked at 619 MB before the fix) | not budgeted                                               |
| Rust process memory over the same scrolls                                                                         | 286 MB before, 337 to 340 MB after and flat (the plugin's 64 MB LRU, the 10,000-entry listing, queue)                                                                                                                                   | 200 MB or less extra at peak: met                          |
| Cold generation of the 10,000 thumbnails (empty cache, debug build, 4 workers, page scrolling a screen at a time) | about 130 per second; each screen of 45 took p50 356 ms, p95 717 ms to appear; 70 s in all                                                                                                                                              | 200+/s for small PNGs: not measured in a release build     |
| A cold fling over about 5,000 tiles (70,000 px in 78 frames, 2.3 s)                                               | 400 thumbnails generated, 140 tiles in the final window; frames p50 16, p95 22, worst 31 ms while it ran; nothing more was generated after it stopped                                                                                   | at most one stale job after a scroll: not measured exactly |

Scrolling is a real cost, but it is paint, not delivery. At fling speeds each frame brings in about 66 new tiles, which the CPU rasteriser draws and WebKit decodes, so the frame rate falls to about 35 Hz where an empty grid holds 60 Hz. Before the fix those pictures simply arrived later (the grid showed icons for 94 to 525 frames of a sweep, against none after), which made the frames lighter and hid the cost; the memory behaves the same way, with the webview peaking at 718 MB before the fix and about 1 GB after it, because more decoded pictures are alive at once. Neither is a regression: what a fling shows is now the pictures.

## `decoding="async"` and `ImageBitmap`

- In the real grid, `<img decoding="async">` and the default were indistinguishable: three alternating runs of each at 200 and 900 px per frame gave the same p95 (21 to 22 ms and 33 to 34 ms), worst frames (31 to 42 ms and 36 to 43 ms) and no frame over 50 ms. WebKitGTK decodes off the main thread already.
- On a synthetic overlay of 60 tiles drawn from `thumb://` (20 rounds each, new pictures each round), all of `<img>` with sync decoding, `<img>` with async decoding, `<img>` after `decode()`, and `fetch` then `createImageBitmap` onto a canvas painted every tile within one or two frames (p50 16 to 17 ms, p95 18 to 32 ms) with a worst frame of 16 to 17 ms. At 16 ms resolution this cannot separate them, and nothing suggests an `ImageBitmap` pool buys anything for 128 px thumbnails.

## Is a virtual list enough?

Yes, as built. The grid keeps 126 to 140 tiles in the DOM at 45 in view (about three screens), whatever the folder size, the loader asks only for what is in view and a screen either side, and at no point did the DOM or the request set grow with the folder: the most tiles in the DOM was 140 over the whole run. `loading="lazy"` has nothing to add to a list that does not render the rows, and was not measured. Cancellation by the loader worked as designed in the cold fling above (400 generated for 5,000 passed), though a worker that has started a thumbnail finishes it, so a fling still generates what the workers reached.

## Not verified

- **A GPU:** the compositor and the webview rasterised on the CPU (pixman, DMABUF off). Frame times while flinging, and the webview's peak memory, will differ with GPU raster; both are the least transferable numbers here.
- **A release build:** the plugin's cold generation (the 200+/s budget), the size of the bridge defect and every Rust-side time are from a debug build.
- **X11, XWayland, KDE, Windows and WebView2** (`http://thumb.localhost`): none was run.
- **A real folder:** the files were on tmpfs; a spinning disk, a cold page cache and files of 12 MP were not tried. The cache hit path read the 79 MB of cache from the page cache, partly through the plugin's 64 MB LRU.
- **Whether WebKitGTK honours `Cache-Control` and `?v=` for the custom scheme:** the scheme already sends `private, max-age=31536000` and the address carries the file's modified time, and the harness's own requests bypassed the cache, so reuse by WebKit was not measured. The hit path is cheap enough (p95 1 ms) that it is not needed for speed.
- **A slow scheme handler and a cancelled in-flight `<img>` request:** the handler answers from memory or one file read, so a slow one could not be provoked without changing it. The loader never cancels a `thumb://` request (it cancels bridge batches), and no leak of requests or elements was seen: the DOM and the webview's memory both levelled off.
- **IPC command counts** (requests, cancels, prioritises per scroll): the bridge's `ipc_monitor` captured nothing from the ES-module client, and `invoke` is frozen in the webview, so they were not counted.
- **Whether the loader's 4,000 kept results or WebKit's own cache decides the webview's roughly 400 MB:** not separated. A smaller `MAX_KEPT` was not tried.
- **Small windows and larger ones:** only 45 visible tiles, not the 60 of the budget; tile size, density and a high-DPI scale were not varied.

## Recommendation

1. **Keep the loader as built** (visible rows plus a screen either side, batches withdrawn when nothing in them is wanted): it held the DOM and the request set flat on a 10,000-image folder and kept 60 Hz at 3,600 px/s.
2. **Keep the bridge fix** and keep batch work in the bridge proportional to the batch. Slice 9 should not resolve entries through selections anywhere else (`thumbnails_request_locations` takes locations and is not affected).
3. **Do not add `decoding="async"` or an `ImageBitmap` pool.** Neither showed any effect.
4. **No new headers are needed.** `Cache-Control: private, max-age=31536000` with the `?v=` modified time is already sent, and serving costs about 1 ms.
5. **The "60 Hz while flinging" budget needs wording.** On this setup a fling over a folder of thumbnails falls to about 35 Hz because of paint and decode of new tiles, while scrolling at 3,600 px/s holds 60 Hz. Slice 26 should measure it on a GPU session before the budget is called met or relaxed; if it stays short, the loader can ask for icons only above a scroll speed (a speed gate in `useViewportThumbnails`) and fill in when the view slows.
6. **Webview memory is about 400 MB above an empty window after a full scroll of 10,000 thumbnails**, and levels off. Slice 26 should record it on the real session; a cap on kept results is the lever if it matters.

## Reproduce

```bash
# the fixture (refuses with less than 3 GB free); delete it afterwards
scripts/thumb-fixture.sh /tmp/waypoint-thumbs
# a private compositor and private data, as for the transparency spike
export XDG_RUNTIME_DIR=/tmp/wp-rt; mkdir -p $XDG_RUNTIME_DIR; chmod 700 $XDG_RUNTIME_DIR
dbus-run-session -- bash -c '
  weston --backend=headless-backend.so --socket=wl-thumbs --width=1280 --height=800 --renderer=pixman &
  sleep 2
  export WAYLAND_DISPLAY=wl-thumbs GDK_BACKEND=wayland XDG_CURRENT_DESKTOP=GNOME
  export XDG_DATA_HOME=/tmp/wp-thumbs/data XDG_CONFIG_HOME=/tmp/wp-thumbs/config XDG_CACHE_HOME=/tmp/wp-thumbs/cache
  bun run tauri:dev'
# memory: sample VmRSS of the app and its WebKitWebProcess children every 500 ms into a file
# (pgrep -f 'target/debug/waypoint$', then each WebKitWebProcess whose parent it is; read /proc/<pid>/status)
```

Then, from the Tauri MCP bridge (`driver_session` start, `webview_execute_js` on the main window; the label is `main-N`), open the folder in the grid and run the harness (a long run must be started without awaiting it, then read its result later, because one script call times out):

```js
const pm = await import('/src/dev/perfHarness.ts');
await pm.openFolder('/tmp/waypoint-thumbs');
await pm.showGrid();
const h = await import('/@fs/<repo>/scripts/thumb-delivery-harness.mjs');
await h.fillCache(); // scrolls a screen at a time: cold generation, or warm passes for memory
await h.timeToPaint(60); // random jumps: time to the first and to every picture in view
await h.serveTimes(); // `thumb://` fetch times, one at a time and 60 at once
await h.sweep(200, 4000); // frame times at 200 px per frame (also 60, 900, 20000)
await h.decodeCompare(); // sync, async, decode() and ImageBitmap on 60 tiles
```

Editing the harness reloads the page (Vite hot update), which loses anything started, so edit it first. For the 10,000 empty files baseline, `scripts/perf-fixture.sh /tmp/x 10000`. Clean up with `rm -rf /tmp/waypoint-thumbs /tmp/wp-thumbs /tmp/wp-rt`.
