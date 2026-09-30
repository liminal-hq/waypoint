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
