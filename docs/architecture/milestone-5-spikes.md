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
