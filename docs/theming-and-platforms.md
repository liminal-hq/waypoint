# Theming & Platforms

## Theme axes

1. **Colour scheme:** Light, Dark or System (the portal's `org.freedesktop.appearance color-scheme`).
2. **Theme source:** _Liminal_ (brand tokens) or _OS_ (portal accent-color plus the GTK/Qt palette hints). This follows the Emoji Nook approach, which detects Adwaita or Breeze.
3. **Accent:** the OS accent, the Liminal ember, or a custom swatch.
4. **Density:** Comfortable or Compact.
5. **Icon theme:** Follow the OS (freedesktop lookup), or Waypoint's own set. The Waypoint set is also the fallback.

## Tokens (semantic)

`bg.window`, `bg.sidebar`, `bg.content`, `bg.raised`, `bg.hover`, `bg.selected`, `border.subtle`, `text.primary`, `text.secondary`, `text.muted`, `accent`, `accent.fg`, `danger`, `success`, `warning`, `focus.ring`.
Brand values come from the liminal-hq site tokens (ember accent, warm neutrals, Space Grotesk for display, a system UI font for body). They're defined per scheme.

## Transparency (headline setting)

| Control                        | Range                                                                                                   | Default                                                |
| ------------------------------ | ------------------------------------------------------------------------------------------------------- | ------------------------------------------------------ |
| Enable transparency            | on/off                                                                                                  | off                                                    |
| Window opacity                 | 40–100 %                                                                                                | 82 %                                                   |
| Background blur                | off / low / high (where the compositor supports it: KWin, Mutter with an extension, Win11 Mica/Acrylic) | low                                                    |
| Regions                        | Title bar · Tabs · Sidebar · Content · Inspector                                                        | Title bar, tabs and sidebar translucent; content solid |
| Menus (context menus, pop-ups) | on/off, own opacity 60–100 %                                                                            | on, 96 %                                               |
| Solid when unfocused           | on/off                                                                                                  | on                                                     |
| Tint                           | Neutral, accent, or wallpaper-derived                                                                   | Neutral                                                |

Guardrails: no part of the window is more see-through than keeps its text at 4.5:1 over the worst backdrop (the floor, a pure function in `theme/transparency.ts`: black behind a light window, a light grey behind a dark one, worked out from the theme's own colours; it comes to roughly 40 to 55 % for the built-in themes, and a pure white wallpaper under a dark window can still dip below). Transparency is disabled under reduced transparency or high contrast, until the platform reports that windows can be see-through, and while the window is not in front when "Solid when unfocused" is on. The live preview in Settings draws a sample window over a bright, busy wallpaper with the same function the real windows use. As built (milestone 5): the regions are three switches (title bar and the rows under it, sidebar and panels, file area) over tiers of the master value (title bar +0, rows +8, sidebar +12, file area +16 points), menus default to solid, the blur setting is Off, Low or High, and the tint control is not built.

**[eng]** Every window is created transparent (`transparent: true`, frameless) whatever the setting, which the rounded corners need; the setting only changes the alpha of the page's backgrounds (`ThemeRoot` writes `--wp-alpha-*` on the root under `data-transparency='on'`, and `tokens.css` mixes the solid `--wp-solid-*` colours with them) and what `src-tauri/src/effects.rs` asks the `window-effects` plugin for (blur on KDE, Mica or Acrylic on Windows). A window root paints nothing while the regions are translucent, so their alphas do not stack. Menus use `backdrop-filter` inside the page.

## DE frames (prototype showcase)

| DE           | Frame treatment                                                       | Controls                           | Accent / type                        |
| ------------ | --------------------------------------------------------------------- | ---------------------------------- | ------------------------------------ |
| GNOME 4x     | libadwaita: rounded 12 px, big header, circular controls on the right | ✕ only by default (layout setting) | GNOME accent, Cantarell/Adwaita Sans |
| Cinnamon     | Mint-Y: 6 px radius, flat title, square controls on the right         | – ▢ ✕                              | Mint green accent, Ubuntu/Noto       |
| KDE Plasma 6 | Breeze: 8 px radius, thin title, round-hover controls                 | – ▢ ✕ (plus Keep Above)            | Breeze blue, Noto Sans               |
| Windows 11   | Mica, 8 px radius, 46 px wide caption buttons, Close hover red        | – ▢ ✕                              | Win accent, Segoe UI Variable        |

Waypoint's own chrome (tabs, sidebar, views) stays the same on every desktop. The frame, controls, fonts, accent, scrollbars and icon theme change. The showcase puts all four side by side on sample wallpapers, each with a light/dark toggle.

### Varying opacity

Regions use tiers of the master opacity, so the structure stays readable: title bar = the master value; tabs slightly more solid; toolbar and inspector more solid again; the file area and sidebar the most solid of the window panels; menus close to solid. The sidebar and file area can be overridden separately. Settings → Transparency has a live preview of the window and of a context menu, each using the same region rules as the app.

## Window frame and shadows

Waypoint draws its own title bar, so its windows are frameless (`decorations: false`) and transparent, and the frame rounds its own corners.

| Platform                     | Shadow                                                                                               | Corners                                                                                  | Notes                                                                                                                                                                                                                                                                                                                                  |
| ---------------------------- | ---------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Linux (GNOME, KDE, Cinnamon) | CSS `box-shadow` inside a 8px transparent margin (`--wp-window-shadow`, `--wp-window-shadow-margin`) | CSS `--wp-window-radius` (12 px)                                                         | GNOME paints no shadow for a frameless window, and Tauri's `shadow` option is unsupported on Linux. The resize border sits in the margin: tao's hit area is a fixed 5px strip at the window's outer edge, so the margin is kept small (8px) to keep the grab zone close to the visible edge, and the shadow is sized to fit inside it. |
| Windows 10 and 11            | The OS (`shadow: true`; DWM)                                                                         | Windows 11 rounds them natively and the CSS radius is 0; Windows 10 keeps the CSS radius | Two mismatched radii would clip against each other, so only one rounds. Windows 11 is detected by build number (22000 and up) because it reports the same `10.0.x` version as Windows 10.                                                                                                                                              |
| macOS (not a target)         | The OS (`shadow: true`)                                                                              | CSS radius                                                                               | Follows the alpha shape of the transparent window. Transparency needs Tauri's `macos-private-api`, which rules out the Mac App Store. Untested.                                                                                                                                                                                        |
| Plain browser (development)  | None                                                                                                 | CSS radius                                                                               | No platform is set, so no margin or shadow is drawn.                                                                                                                                                                                                                                                                                   |

On Linux the shadow is removed while the window is unfocused (`--wp-window-shadow-unfocused`, the hairline ring only), which makes the active window stand out when moving between windows. A maximised window has no margin, shadow, rounding or border on every platform. `data-platform` on the root element comes from `@tauri-apps/plugin-os`; the per-platform tokens live in `apps/waypoint/src/theme/tokens.css`. The `window-effects` plugin's `set_shadow_inset` calls `gdk_window_set_shadow_width` with the 8 px margin on every framed window (and zero while it is maximised or full screen), so the compositor leaves the margin out of the window's geometry and a tiled window should sit flush: `src-tauri/src/effects.rs` sets it when a window's page starts loading and after every resize. It is unavailable, with a reason, where the window is not a GTK one. **Status:** the call is made and succeeds on GNOME-style Wayland (a headless Weston, with no tiling to try), and a maximised window is flush; whether a half-tiled window now sits flush on GNOME, KDE and X11 has not been looked at, so the previous limitation stands until it has (see `milestone-5-spikes.md`).
