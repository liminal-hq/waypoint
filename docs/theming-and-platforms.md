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

Guardrails: text sits on at least a 70 % solid backing, or on a local scrim if the measured contrast falls below 4.5:1. Transparency is disabled under reduced transparency or high contrast. The live preview in Settings shows the real window over a sample wallpaper.

**[eng]** Tauri `transparent: true` with the window vibrancy plugin, and CSS `backdrop-filter` inside the app.

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

On Linux the shadow is removed while the window is unfocused (`--wp-window-shadow-unfocused`, the hairline ring only), which makes the active window stand out when moving between windows. A maximised window has no margin, shadow, rounding or border on every platform. `data-platform` on the root element comes from `@tauri-apps/plugin-os`; the per-platform tokens live in `apps/waypoint/src/theme/tokens.css`. Known limitation on Linux: a half-tiled window keeps its transparent margin, because the page cannot tell the compositor to exclude the margin from the window geometry. The `window-effects` plugin's `set_shadow_inset` is the native fix: it calls `gdk_window_set_shadow_width` with the margin, so the compositor leaves it out of the window's geometry and a tiled window sits flush; the app turns it on in a later slice, and it is unavailable (with a reason) where the window is not a GTK one.
