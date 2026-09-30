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

**Not measured:** WebView2 on Windows 11 (the VM run only checked that the app builds and lists folders), a maximised window, a cold cache, the grid view, memory per open listing, and the sidebar's folders tree.
