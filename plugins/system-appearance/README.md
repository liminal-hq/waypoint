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
| Linux    | KDE: `kdeglobals` (`kdeglobals`): `[Colors:Window] BackgroundNormal` for the scheme, `[General] AccentColor`, `[KDE] AnimationDurationFactor` (0 is reduced motion), `[General] forceFontDPI` over 96 for the text scale, `[Icons] Theme`                                                                                                                            | A file watcher on the config directory, as for `kwinrc`                                                   |
| Linux    | Cinnamon: `org.x.apps.portal` or `org.gnome.desktop.interface` `color-scheme`, and `org.cinnamon.desktop.interface` (`enable-animations`, `text-scaling-factor`, `icon-theme`) and `org.cinnamon.desktop.a11y.interface` (`high-contrast`) through `gsettings`                                                                                                       | `gsettings monitor` on each schema, and the portal signal                                                 |
| Linux    | MATE, Xfce: the portal only                                                                                                                                                                                                                                                                                                                                          | The portal signal                                                                                         |
| Windows  | `UISettings` (`uiSettings`): `Accent`, `AnimationsEnabled`, `AdvancedEffectsEnabled` (off is reduced transparency) and `TextScaleFactor`; `AccessibilitySettings.HighContrast` (`accessibilitySettings`); registry `AppsUseLightTheme` (`registry`)                                                                                                                  | `UISettings` and `AccessibilitySettings` change events, and a ten-second poll for what no event announces |
| macOS    | Every feature reports `platformUnsupported`                                                                                                                                                                                                                                                                                                                          | Nothing                                                                                                   |

No source offers reduced transparency on Linux, high contrast or reduced transparency on KDE, or an accent colour on Cinnamon; those report `noSource`. Windows has no icon theme. Reading is best-effort: KDE, Cinnamon, MATE, Xfce and Windows are written to the documented keys and APIs and covered by tests on sample data, but have not been run on those systems.

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

## Permissions

The `default` permission set allows all three commands. Registering the plugin is not enough: Tauri denies every command until a capability grants it, so each window that uses the plugin needs the permission in a capability file, for example `src-tauri/capabilities/default.json`:

```json
{
	"identifier": "default",
	"windows": ["main"],
	"permissions": ["system-appearance:default", "core:default"]
}
```

`core:default` includes the event permissions that `onTitlebarPreferencesChanged` and `onAppearanceChanged` need to listen for the change event. Without these, every JavaScript call rejects with a "not allowed" error. Grant them only to the windows that need them; a wildcard scope also covers windows you add later.

| Permission                       | Command                    |
| -------------------------------- | -------------------------- |
| `allow-get-status`               | `get_status`               |
| `allow-get-titlebar-preferences` | `get_titlebar_preferences` |
| `allow-get-appearance`           | `get_appearance`           |

## Development

The parsing of every desktop's titlebar format lives in `src/parse.rs`, and that of the appearance values (portal variants, accent colours, `kdeglobals`, `gsettings` text, Windows values) in `src/appearance/parse.rs`, as pure functions with table-driven tests; `src/appearance/resolve.rs` holds the precedence between sources. The per-platform readers and watchers are thin modules in `src/linux/`, `src/windows.rs`, `src/macos.rs` and `src/appearance/`. To regenerate the TypeScript bindings, run `cargo test` in the plugin directory; to smoke-test the live portal on Linux, run `cargo test live_portal_read -- --ignored --nocapture`, and to print what this machine's appearance resolves to (read-only), run `cargo test live_appearance_read -- --ignored --nocapture` on Linux or Windows.

## Licence

Licensed under either of Apache License 2.0 or MIT licence at your option.
