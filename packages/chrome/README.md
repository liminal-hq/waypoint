# @liminal-hq/waypoint-chrome

Shared Liminal HQ window chrome: a unified title bar, the window menu and a context menu. React 19 and TypeScript, with no file-manager concepts — the package knows nothing about Waypoint, so it can be extracted for other Liminal apps. The host window is reached only through the `WindowControls` adapter, and the components ship as source (no build step, no barrel file).

## Deep-path imports

Every export maps to the file that defines it.

| Import                                                     | Provides                                                                                           |
| ---------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| `@liminal-hq/waypoint-chrome/TitleBar`                     | `TitleBar` and its props: `start`, `center`, `end` slots plus built-in window buttons              |
| `@liminal-hq/waypoint-chrome/TitleBar/AppMenuButton`       | App mark and menu button, opened by click, `F10` or a lone `Alt` press                             |
| `@liminal-hq/waypoint-chrome/TitleBar/useMaximised`        | `useMaximised(controls)` hook                                                                      |
| `@liminal-hq/waypoint-chrome/TitleBar/windowControls`      | `WindowControls`, `ControlsStyle`, `ControlsSide` types                                            |
| `@liminal-hq/waypoint-chrome/TitleBar/tauriWindowControls` | `createTauriWindowControls()` and a shared `tauriWindowControls` built on `@tauri-apps/api/window` |
| `@liminal-hq/waypoint-chrome/ContextMenu`                  | Controlled `ContextMenu`                                                                           |
| `@liminal-hq/waypoint-chrome/ContextMenu/types`            | `MenuItem` and related types                                                                       |
| `@liminal-hq/waypoint-chrome/WindowMenu`                   | `WindowMenu`, the window menu bound to a `WindowControls`                                          |
| `@liminal-hq/waypoint-chrome/WindowMenu/windowMenuModel`   | `buildWindowMenuModel()`, the pure menu model                                                      |
| `@liminal-hq/waypoint-chrome/labels`                       | `ChromeLabels` and `defaultChromeLabels`                                                           |
| `@liminal-hq/waypoint-chrome/icons/icons`                  | Inline SVG icons used by the chrome                                                                |

## The `WindowControls` adapter

The title bar never imports a window API. The host supplies an object with this shape.

```ts
interface WindowControls {
	minimize(): void | Promise<void>;
	toggleMaximize(): void | Promise<void>;
	close(): void | Promise<void>;
	startDragging?(): void | Promise<void>; // enables "Move" in the window menu
	setAlwaysOnTop(value: boolean): void | Promise<void>;
	isAlwaysOnTop?(): boolean | Promise<boolean>; // initial state, false when omitted
	isMaximized(): boolean | Promise<boolean>;
	onMaximizedChange(listener: (maximised: boolean) => void): () => void; // returns unsubscribe
	handlesDoubleClickNatively?: boolean; // true when the host already maximises on double-click
}
```

Pass a stable object (create it once at module level or memoise it) because the hooks subscribe per identity. Tauri's drag-region script already maximises on double-click, so `tauriWindowControls` sets `handlesDoubleClickNatively` and the title bar avoids toggling twice.

## Title bar

```tsx
<TitleBar
	windowControls={tauriWindowControls}
	controlsStyle="gnome" // 'gnome' | 'kde' | 'win11' | 'cinnamon'
	controlsSide="end" // 'start' | 'end'
	showAlwaysOnTop
	start={<AppMenuButton label="App" items={menu} onSelect={run} />}
	center={<span>Title</span>}
	end={<button type="button">Action</button>}
/>
```

- The controls style and side are data-driven: the package never reads the operating system, so a host plugin supplies them.
- `data-tauri-drag-region` is set on the bar and on the passive slot wrappers, never on buttons or the controls group.
- The bar gets a `maximised` class (square corners) while the window is maximised.
- `transparent` lets the desktop show through, with opacity from `--wp-title-bar-opacity`.
- Right-clicking empty bar space opens the window menu — Restore or Maximise, Minimise, Move, Always on Top, Close. Buttons, links, inputs, and anything marked `data-window-menu-exclude` (the app menu button) are excluded.

## Context menu

`ContextMenu` is controlled: pass `items`, a viewport `position`, `onSelect` and `onClose`. Item types are `action` (with an optional `danger` flag), `checkbox`, `submenu`, `separator` and `section`. It follows the WAI-ARIA menu pattern — `role="menu"`, `menuitem` and `menuitemcheckbox`, arrow keys, `Home` and `End`, `Enter` and `Space`, `Esc`, type-ahead, submenus opened by `ArrowRight` or after 150 ms of hover — clamps itself to the viewport, and returns focus to the element that was focused when it opened (or to `returnFocusTo`).

## Tokens

Styling uses CSS custom properties with fallbacks, so the package renders before an app defines them: `--wp-bg-raised`, `--wp-bg-chrome`, `--wp-border-subtle`, `--wp-text-primary`, `--wp-text-muted`, `--wp-accent`, `--wp-accent-contrast`, `--wp-danger`, `--wp-focus-ring`, `--wp-control-bg`, `--wp-control-bg-hover`, `--wp-title-bar-height`, `--wp-title-bar-opacity`, `--wp-window-radius`, `--wp-font-mono`, `--wp-shadow-menu` and `--wp-z-menu`.

## Localisation

Components take a `labels` prop (`ChromeLabels`) and default to English strings from `defaultChromeLabels`; the app supplies translated values from its message catalogue.
