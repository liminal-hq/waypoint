# @liminal-hq/plugin-system-appearance

Reads the operating system's window-titlebar preferences — which buttons appear on which side of the title, and what double-, middle- and right-clicking the titlebar does — and its appearance preferences — colour scheme, accent colour, contrast, reduced motion and transparency, text scale and icon theme — and pushes changes to every window, so custom titlebars and themes can follow the user's desktop.

## Installation

### Rust

```toml
[dependencies]
tauri-plugin-system-appearance = "0.1"

# Alternatively with Git:
tauri-plugin-system-appearance = { git = "https://github.com/liminal-hq/tauri-plugins-workspace", branch = "main" }
```

### JavaScript

```bash
pnpm add @liminal-hq/plugin-system-appearance
```

## Usage

### Rust

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_system_appearance::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### JavaScript

```typescript
import {
	getAppearance,
	getStatus,
	getTitlebarPreferences,
	onAppearanceChanged,
	onTitlebarPreferencesChanged,
} from '@liminal-hq/plugin-system-appearance';

const status = await getStatus();
const preferences = await getTitlebarPreferences();

const unlisten = await onTitlebarPreferencesChanged((next) => {
	console.log(next.buttonLayout, next.actions);
});

const stop = await onAppearanceChanged((next) => {
	console.log(next.colourScheme, next.accent, next.sources);
});
const appearance = await getAppearance();
```

`getTitlebarPreferences()` never rejects because a source is unavailable: it returns the default preferences with `source: 'default'` instead. The plugin remembers the last preferences it read and emits `system-appearance://titlebar-preferences-changed` only when the value actually changes.

Both the read and the event carry a `revision`, a counter that starts at 1 and increases exactly when the preferences change. A change event and a read can arrive in either order, so keep the highest revision you have seen and ignore anything older. To avoid missing a change made while starting up, subscribe first and read second.

## Appearance preferences

`getAppearance()` resolves to the colour scheme (`light`, `dark` or `noPreference`), the accent colour as lower-case `#rrggbb` (or `null`), the contrast (`normal` or `more`), whether reduced motion and reduced transparency are asked for, the text scale as a multiplier (1 is the default size), the icon-theme name (or `null`) and a `sources` object that says which source supplied each one. `onAppearanceChanged` delivers the same object, with a higher `revision`, whenever a value or a source changes. The revision works as it does for the titlebar preferences: start at 1, increase on change, keep the highest seen, and subscribe before reading.

A preference that nothing could answer holds a neutral value (`noPreference`, `null`, `normal`, `false` or a text scale of 1) and its entry in `sources` is `null`. Do not take the neutral value for the user's choice: `getStatus().appearanceAvailable` says whether any of them works (`getStatus().available` keeps describing the titlebar preferences only), and `getStatus().appearance` lists every feature (`colourScheme`, `accent`, `contrast`, `reducedMotion`, `reducedTransparency`, `textScale`, `iconTheme`) with `available`, the `source` it comes from, and, when it does not work, a typed `reason` (`platformUnsupported`, `noSource`, `portalUnavailable`, `toolMissing`, `sourceMissing` or `readFailed`) and a `detail` string. Hide options whose feature is unavailable.

Where several sources exist they are tried from the most to the least authoritative, the first answer wins, and the winner is recorded in `sources`.

| Platform | Source, in order                                                                                                                                                                                                                                                                                                                                                     | Watched by                                                                                                |
| -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| Linux    | xdg-desktop-portal Settings (`portal`): `org.freedesktop.appearance` `color-scheme`, `accent-color`, `contrast` and `reduced-motion`, then the keys the portal passes through from `org.gnome.desktop.interface` (`enable-animations`, `text-scaling-factor`, `icon-theme`, `color-scheme`, `accent-color`) and `org.gnome.desktop.a11y.interface` (`high-contrast`) | The portal `SettingChanged` signal                                                                        |
| Linux    | GNOME family and unrecognised desktops: the same GNOME keys through `gsettings`, asked only for what the portal did not answer                                                                                                                                                                                                                                       | The portal signal only                                                                                    |
| Linux    | KDE: `kdeglobals` (`kdeglobals`): `[Colors:Window] BackgroundNormal` for the scheme, `[General] AccentColor`, `[KDE] AnimationDurationFactor` (0 is reduced motion), `[Icons] Theme`, and in the separate `kcmfonts` file `[General] forceFontDPI` over 96 for the text scale (unanswered when it is not set)                                                        | A file watcher on the config directory (`kdeglobals` and `kcmfonts`), as for `kwinrc`                     |
| Linux    | Cinnamon: `org.x.apps.portal` or `org.gnome.desktop.interface` `color-scheme`, and `org.cinnamon.desktop.interface` (`enable-animations`, `text-scaling-factor`, `icon-theme`) and `org.cinnamon.desktop.a11y.interface` (`high-contrast`) through `gsettings`                                                                                                       | `gsettings monitor` on each schema, and the portal signal                                                 |
| Linux    | MATE, Xfce: the portal only                                                                                                                                                                                                                                                                                                                                          | The portal signal                                                                                         |
| Windows  | `UISettings` (`uiSettings`): `Accent`, `AnimationsEnabled`, `AdvancedEffectsEnabled` (off is reduced transparency) and `TextScaleFactor`; `AccessibilitySettings.HighContrast` (`accessibilitySettings`); registry `AppsUseLightTheme` (`registry`)                                                                                                                  | `UISettings` and `AccessibilitySettings` change events, and a ten-second poll for what no event announces |
| macOS    | Every feature reports `platformUnsupported`                                                                                                                                                                                                                                                                                                                          | Nothing                                                                                                   |

No source offers reduced transparency on Linux, high contrast or reduced transparency on KDE, or an accent colour on Cinnamon; those report `noSource`. Windows has no icon theme. Reading is best-effort: KDE, Cinnamon, MATE, Xfce and Windows are written to the documented keys and APIs and covered by tests on sample data, but have not been run on those systems.

## Colour palette

`getPalette()` resolves to the operating system's colours: `windowBackground` and `windowForeground`, `viewBackground` and `viewForeground` (lists, text fields), `surfaceBackground` (popovers, cards, buttons), `selectionBackground` and `selectionForeground`, `border`, `focus`, `warning`, `error` and `success`, and the title bar's two tones, `titleBarBackground` (the top: a GTK theme's `headerbar_bg_color`, KDE's `[Colors:Header]`, the accent on Windows when accent colour on title bars is on) and `titleBarBackgroundEnd` (the bottom: the theme's header bar shade, KDE's `BackgroundAlternate`, the gradient caption colour in Windows high contrast; only reported with a top). Each is a `PaletteEntry`: `colour` as lower-case `#rrggbb`, the `source` that supplied it (`gtkTheme`, `kdeGlobals`, `portal`, `uiSettings` or `sysColor`), or, when it is unavailable, a typed `reason` (the same values as the appearance features) and a `detail` that says what was missing. `status.available` is true when the window background and the text on it are both known, which is the least a consumer needs to restyle a window; `status.source` and `status.reason` say where they came from or why not. `onPaletteChanged` delivers the same object, with a higher `revision`, whenever a colour or its source changes; the revision works as it does for the preferences above, and `getStatus().palette` carries the same status so an options page can hide what cannot work.

| Platform | Source, in order                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    | Watched by                                                                                                              |
| -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| Linux    | KDE: `kdeglobals` (`kdeGlobals`): `[Colors:Window]`, `[Colors:View]`, `[Colors:Button]` and `[Colors:Selection]` `BackgroundNormal` and `ForegroundNormal`, `[Colors:Window] DecorationFocus`, and `[Colors:View]` `ForegroundNeutral`, `ForegroundNegative` and `ForegroundPositive` for the warning, error and success colours; a profile with no colour sections falls back to the GTK theme                                                                                                                                     | A file watcher on the config directory, and the portal signal                                                           |
| Linux    | Everywhere else, and KDE's fallback: the GTK theme's named colours (`gtkTheme`) through `StyleContext::lookup_color`: `theme_bg_color`, `theme_fg_color`, `theme_base_color`, `theme_text_color`, `theme_selected_bg_color`, `theme_selected_fg_color`, `borders`, `warning_color`, `error_color`, `success_color`, and, on libadwaita-style themes, `popover_bg_color` (or `card_bg_color`) and `accent_bg_color`. A name the theme lacks is a `sourceMissing` entry; a translucent colour is flattened over the window background | The portal signal, and GTK's `gtk-theme-name`, `gtk-application-prefer-dark-theme` and `gtk-color-scheme` notifications |
| Linux    | The portal's accent colour (`portal`) fills `selectionBackground` and `focus` when the theme gave none                                                                                                                                                                                                                                                                                                                                                                                                                              | The portal signal                                                                                                       |
| Windows  | `UISettings.GetColorValue` (`uiSettings`): `Background` and `Foreground` for the window and view colours, `Accent` for the selection and focus; in a high-contrast theme `GetSysColor` (`sysColor`) overrides them with the user's own colours (`COLOR_WINDOW`, `COLOR_WINDOWTEXT`, `COLOR_BTNFACE`, `COLOR_HIGHLIGHT`, `COLOR_HIGHLIGHTTEXT`, `COLOR_WINDOWFRAME`, `COLOR_HOTLIGHT`)                                                                                                                                               | `UISettings.ColorValuesChanged` and `AccessibilitySettings.HighContrastChanged`                                         |
| macOS    | Every colour reports `platformUnsupported`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | Nothing                                                                                                                 |

KDE has no border colour in `kdeglobals`, and Windows has no surface, selection text, border or status colours outside a high-contrast theme; those report `noSource` or `sourceMissing`. Reading is best-effort: the mapping is covered by tests on sample data (a `kdeglobals` file and a table of GTK named colours), and the GTK reader has been run on one desktop, but KDE and Windows have not been run on those systems.

## Sources

| Platform | Desktop environment                                       | Source                                                                                                  | Watched by                             |
| -------- | --------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- | -------------------------------------- |
| Linux    | GNOME family (GNOME, Budgie, Pantheon, Unity) and unknown | xdg-desktop-portal Settings, namespace `org.gnome.desktop.wm.preferences` (`portal`)                    | The portal `SettingChanged` signal     |
| Linux    | KDE                                                       | `kwinrc` in `XDG_CONFIG_HOME` or `~/.config` (`kwin-config`)                                            | A file watcher on the config directory |
| Linux    | Cinnamon                                                  | `gsettings` schema `org.cinnamon.desktop.wm.preferences` (`gsettings`)                                  | `gsettings monitor`                    |
| Linux    | MATE                                                      | `gsettings` schema `org.mate.Marco.general` (`gsettings`)                                               | `gsettings monitor`                    |
| Linux    | XFCE                                                      | `xfconf-query -c xfwm4` (`xfconf`)                                                                      | `xfconf-query -c xfwm4 -m`             |
| Windows  | Windows                                                   | Fixed platform value (`platform`)                                                                       | Nothing; the value is constant         |
| macOS    | macOS                                                     | Fixed layout, with the double-click action from `defaults read NSGlobalDomain AppleActionOnDoubleClick` | Nothing; re-read on each command call  |

The desktop environment on Linux comes from `XDG_CURRENT_DESKTOP`, which may hold several colon-separated names. If a source cannot be read, the plugin reports the default: no buttons at the start, `minimise`, `maximise` and `close` at the end, double-click toggles maximise, middle-click does nothing and right-click opens the menu.

## Types

All JSON is camelCase. TypeScript bindings are generated by ts-rs into `guest-js/bindings`.

```typescript
type WindowButton =
	| 'appMenu'
	| 'windowMenu'
	| 'minimise'
	| 'maximise'
	| 'close'
	| 'keepAbove'
	| 'keepBelow'
	| 'shade'
	| 'stick'
	| 'help';

// Order matters; either side may be empty.
interface ButtonLayout {
	start: WindowButton[];
	end: WindowButton[];
}

type TitlebarAction =
	| 'toggleMaximise'
	| 'toggleMaximiseHorizontally' // GNOME `toggle-maximize-horizontally`, KDE `Maximize (horizontal only)`
	| 'toggleMaximiseVertically' // GNOME `toggle-maximize-vertically`, KDE `Maximize (vertical only)`
	| 'toggleShade'
	| 'minimise'
	| 'lower'
	| 'toggleRaiseLower' // KDE `Toggle raise and lower`
	| 'raise' // KDE `Raise`
	| 'toggleAllDesktops' // KDE `OnAllDesktops`
	| 'toggleAbove' // XFCE `above`
	| 'fill' // XFCE `fill`
	| 'close' // KDE `Close`
	| 'menu'
	| 'none';

interface TitlebarActions {
	doubleClick: TitlebarAction;
	middleClick: TitlebarAction;
	rightClick: TitlebarAction;
}

type DesktopEnvironment =
	'gnome' | 'kde' | 'cinnamon' | 'mate' | 'xfce' | 'windows' | 'macos' | 'unknown';

type LayoutSource = 'portal' | 'kwinConfig' | 'gsettings' | 'xfconf' | 'platform' | 'default';

// What `getTitlebarPreferences` resolves to and the change event carries: the preferences below
// plus a revision.
interface TitlebarSnapshot extends TitlebarPreferences {
	revision: number;
}

interface TitlebarPreferences {
	buttonLayout: ButtonLayout;
	actions: TitlebarActions;
	desktopEnvironment: DesktopEnvironment;
	source: LayoutSource;
}

// `available`, `reason` and `features` describe the titlebar preferences: `available` is true when
// a titlebar source answered, `reason` says why none did, and `features` names the sources that
// worked ('portal', 'kwin-config', 'gsettings', 'xfconf' or 'platform'). `appearanceAvailable` is
// true when any appearance feature works, and `appearance` reports each one.
interface PluginStatus {
	available: boolean;
	reason: string | null;
	features: string[];
	appearanceAvailable: boolean;
	appearance: AppearanceFeatureStatus[];
	palette: PaletteStatus;
}

type ColourScheme = 'light' | 'dark' | 'noPreference';
type Contrast = 'normal' | 'more';
type AppearanceSource =
	'portal' | 'gsettings' | 'kdeGlobals' | 'uiSettings' | 'accessibilitySettings' | 'registry';

// The same keys as `AppearanceValues.sources` below.
type AppearanceSources = Record<keyof Omit<AppearanceValues, 'sources'>, AppearanceSource | null>;

// What `getAppearance` resolves to and the change event carries: the values below plus a revision.
interface AppearancePreferences extends AppearanceValues {
	revision: number;
}

interface AppearanceValues {
	colourScheme: ColourScheme;
	accent: string | null; // `#rrggbb`
	contrast: Contrast;
	reducedMotion: boolean;
	reducedTransparency: boolean;
	textScale: number; // 1 is the default size
	iconTheme: string | null;
	sources: AppearanceSources;
}

type AppearanceFeature =
	| 'colourScheme'
	| 'accent'
	| 'contrast'
	| 'reducedMotion'
	| 'reducedTransparency'
	| 'textScale'
	| 'iconTheme';

type UnavailableReason =
	| 'platformUnsupported'
	| 'noSource'
	| 'portalUnavailable'
	| 'toolMissing'
	| 'sourceMissing'
	| 'readFailed';

interface AppearanceFeatureStatus {
	feature: AppearanceFeature;
	available: boolean;
	source: AppearanceSource | null;
	reason: UnavailableReason | null;
	detail: string | null;
}
```

```typescript
type PaletteSource = 'gtkTheme' | 'kdeGlobals' | 'portal' | 'uiSettings' | 'sysColor';

interface PaletteEntry {
	colour: string | null; // `#rrggbb`
	source: PaletteSource | null;
	reason: UnavailableReason | null;
	detail: string | null;
}

interface PaletteStatus {
	available: boolean;
	source: PaletteSource | null;
	reason: UnavailableReason | null;
	detail: string | null;
}

// What `getPalette` resolves to and the change event carries.
interface Palette extends PaletteColours {
	revision: number;
	status: PaletteStatus;
}

// Every colour is a PaletteEntry.
interface PaletteColours {
	windowBackground: PaletteEntry;
	windowForeground: PaletteEntry;
	viewBackground: PaletteEntry;
	viewForeground: PaletteEntry;
	surfaceBackground: PaletteEntry;
	selectionBackground: PaletteEntry;
	selectionForeground: PaletteEntry;
	border: PaletteEntry;
	focus: PaletteEntry;
	warning: PaletteEntry;
	error: PaletteEntry;
	success: PaletteEntry;
	titleBarBackground: PaletteEntry;
	titleBarBackgroundEnd: PaletteEntry;
}
```

## Permissions

The `default` permission set allows all four commands. Registering the plugin is not enough: Tauri denies every command until a capability grants it, so each window that uses the plugin needs the permission in a capability file, for example `src-tauri/capabilities/default.json`:

```json
{
	"identifier": "default",
	"windows": ["main"],
	"permissions": ["system-appearance:default", "core:default"]
}
```

`core:default` includes the event permissions that `onTitlebarPreferencesChanged`, `onAppearanceChanged` and `onPaletteChanged` need to listen for the change event. Without these, every JavaScript call rejects with a "not allowed" error. Grant them only to the windows that need them; a wildcard scope also covers windows you add later.

| Permission                       | Command                    |
| -------------------------------- | -------------------------- |
| `allow-get-status`               | `get_status`               |
| `allow-get-titlebar-preferences` | `get_titlebar_preferences` |
| `allow-get-appearance`           | `get_appearance`           |
| `allow-get-palette`              | `get_palette`              |

## Development

The parsing of every desktop's titlebar format lives in `src/parse.rs`, and that of the appearance values (portal variants, accent colours, `kdeglobals`, `gsettings` text, Windows values) in `src/appearance/parse.rs`, as pure functions with table-driven tests; `src/appearance/resolve.rs` holds the precedence between sources. The per-platform readers and watchers are thin modules in `src/linux/`, `src/windows.rs`, `src/macos.rs` and `src/appearance/`. The palette's mapping (GTK named colours, `kdeglobals` sections, Windows colour values) lives in `src/palette/parse.rs` with the same kind of tests, and its readers are in `src/palette/`. To regenerate the TypeScript bindings, run `cargo test` in the plugin directory; to smoke-test the live portal on Linux, run `cargo test live_portal_read -- --ignored --nocapture`, and to print what this machine's appearance resolves to (read-only), run `cargo test live_appearance_read -- --ignored --nocapture` on Linux or Windows; the palette's Windows reader has `live_palette_read`.

## Licence

Licensed under either of Apache License 2.0 or MIT licence at your option.
