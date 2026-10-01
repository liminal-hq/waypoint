# @liminal-hq/waypoint-chrome

Shared Liminal HQ window chrome: a unified title bar, the window menu and a context menu. React 19 and TypeScript, with no file-manager concepts — the package knows nothing about Waypoint, so it can be extracted for other Liminal apps. The host window is reached only through the `WindowControls` adapter, and the components ship as source (no build step, no barrel file).

## Deep-path imports

Every export maps to the file that defines it.

| Import                                                                  | Provides                                                                                           |
| ----------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| `@liminal-hq/waypoint-chrome/TitleBar`                                  | `TitleBar` and its props: `start`, `center`, `end` slots plus built-in window buttons              |
| `@liminal-hq/waypoint-chrome/TitleBar/TitleBarTitle`                    | `TitleBarTitle`: draggable, truncating title text for the `center` slot                            |
| `@liminal-hq/waypoint-chrome/TitleBar/buttonLayout`                     | `ButtonLayout`, `TitlebarActions` types and their `DEFAULT_*` values                               |
| `@liminal-hq/waypoint-chrome/WindowChromeProvider/WindowChromeProvider` | `WindowChromeProvider`, `useWindowControls`, `useWindowMaximised`, `useWindowFocused`              |
| `@liminal-hq/waypoint-chrome/TitleBar/AppMenuButton`                    | App mark and menu button, opened by click, `F10` or a lone `Alt` press                             |
| `@liminal-hq/waypoint-chrome/WindowFrame`                               | `WindowFrame`: rounded, clipped, shadowed surface for a frameless window                           |
| `@liminal-hq/waypoint-chrome/TitleBar/windowControls`                   | `WindowControls` and `ControlsStyle` types                                                         |
| `@liminal-hq/waypoint-chrome/TitleBar/tauriWindowControls`              | `createTauriWindowControls()` and a shared `tauriWindowControls` built on `@tauri-apps/api/window` |
| `@liminal-hq/waypoint-chrome/ContextMenu`                               | Controlled `ContextMenu`                                                                           |
| `@liminal-hq/waypoint-chrome/ContextMenu/types`                         | `MenuItem` and related types                                                                       |
| `@liminal-hq/waypoint-chrome/Dialog/Dialog`                             | `Dialog`: controlled modal over the native `<dialog>`                                              |
| `@liminal-hq/waypoint-chrome/Dialog/DialogActions`                      | `DialogActions` and `DialogButton` for the footer                                                  |
| `@liminal-hq/waypoint-chrome/Dialog/ConfirmDialog`                      | `ConfirmDialog`: title, message, confirm and cancel                                                |
| `@liminal-hq/waypoint-chrome/Dialog/dialogContext`                      | `DialogCloseReason` type                                                                           |
| `@liminal-hq/waypoint-chrome/WindowMenu`                                | `WindowMenu`, the window menu bound to the provider's controls                                     |
| `@liminal-hq/waypoint-chrome/WindowMenu/windowMenuModel`                | `buildWindowMenuModel()`, the pure menu model                                                      |
| `@liminal-hq/waypoint-chrome/labels`                                    | `ChromeLabels` and `defaultChromeLabels`                                                           |
| `@liminal-hq/waypoint-chrome/icons/icons`                               | Inline SVG icons used by the chrome                                                                |

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
	showSystemMenu?(position: { x: number; y: number }): boolean | void | Promise<boolean | void>; // opens the compositor's own window menu
	isMaximized(): boolean | Promise<boolean>;
	onMaximizedChange(listener: (maximised: boolean) => void): Unsubscribe;
	isFocused?(): boolean | Promise<boolean>; // initial state, true when omitted
	onFocusChange?(listener: (focused: boolean) => void): Unsubscribe;
	handlesDoubleClickNatively?: boolean; // true when the host already maximises on double-click
}
```

`Unsubscribe` is `(() => void) & { ready?: Promise<void> }`. A host whose listeners register asynchronously (Tauri's do) sets `ready`, which resolves once the listener is live. `WindowChromeProvider` subscribes first and reads the current state only after `ready`, and it discards a read that resolves after a change event already arrived, so a maximise or focus change can neither slip between the read and the subscription nor be overwritten by an older read.

Pass a stable object (create it once at module level or memoise it) because `WindowChromeProvider` subscribes per identity. Tauri's drag-region script already maximises on double-click, so `tauriWindowControls` sets `handlesDoubleClickNatively` and the title bar avoids toggling twice.

## Window chrome provider

`WindowChromeProvider` is the single source of window state. It takes the `WindowControls` adapter, reads the initial maximised and focus state, subscribes once to `onMaximizedChange` and `onFocusChange`, and shares `{ controls, maximised, focused }` through context. Mount it once near the root; `TitleBar`, `WindowFrame`, `WindowMenu` and `ContextMenu` all read from it, so the adapter is subscribed exactly once however many chrome components are mounted.

```tsx
<WindowChromeProvider controls={tauriWindowControls}>
	<WindowFrame>
		<TitleBar />
	</WindowFrame>
</WindowChromeProvider>
```

- `useWindowControls()`, `useWindowMaximised()` and `useWindowFocused()` read the shared values for app code that needs them. They throw outside a provider.
- `TitleBar`, `WindowFrame` and `WindowMenu` throw a clear error when no provider is mounted. `ContextMenu` works without one.
- There is no `windowControls` prop on any component.

## Title bar

```tsx
<TitleBar
	controlsStyle="gnome" // 'gnome' | 'kde' | 'win11' | 'cinnamon'
	buttonLayout={{ start: [], end: ['minimise', 'maximise', 'close'] }}
	titlebarActions={{ doubleClick: 'toggleMaximise', middleClick: 'none', rightClick: 'menu' }}
	titleAlign="center" // 'center' | 'start'
	showAlwaysOnTop
	start={<AppMenuButton label="App" items={menu} onSelect={run} />}
	center={<TitleBarTitle>Title</TitleBarTitle>}
	end={<button type="button">Action</button>}
/>
```

- The controls style, button layout and titlebar actions are data-driven: the package never reads the operating system, so a host plugin supplies them.
- `data-tauri-drag-region` is set on the bar, the three groups and the passive slot wrappers, never on buttons or the controls groups. Tauri's drag handler checks only the pressed element, not its ancestors, so any static text or other non-interactive element you place in a slot needs `data-tauri-drag-region` itself. `TitleBarTitle` is a `<span>` that carries it and truncates to a single line with an ellipsis.
- The bar gets a `maximised` class (square corners) while the window is maximised.
- `transparent` lets the desktop show through, with opacity from `--wp-title-bar-opacity`.

### Layout and centring

The bar is a three-column CSS grid, `minmax(0, 1fr) auto minmax(0, 1fr)`, holding a start group, the centre and an end group. The two outer columns are equal, so the centre is exactly centred in the window whichever buttons sit on either side, and a long title truncates inside the centre. The DOM is always the same three children:

- Start group (`data-group="start"`): the start-side window buttons, then the `start` slot, flush left.
- Centre (`data-group="centre"`): the `center` slot.
- End group (`data-group="end"`): the `end` slot, then the end-side window buttons, flush right.

`titleAlign="start"` (`data-title-align="start"`) switches the grid to `auto minmax(0, 1fr) auto` and left-aligns the centre content directly after the start group, the Windows convention. The default, `center`, keeps the exact centring.

### Button layout

`buttonLayout` is `{ start: ChromeButton[]; end: ChromeButton[] }`, structurally the same as the JSON an appearance plugin reports. Tokens are `appMenu`, `windowMenu`, `minimise`, `maximise`, `close`, `keepAbove`, `keepBelow`, `shade`, `stick` and `help`. The default, `DEFAULT_BUTTON_LAYOUT`, is `{ start: [], end: ['minimise', 'maximise', 'close'] }`.

- The chrome renders `minimise`, `maximise` (with a restore glyph while maximised), `close` and `keepAbove` (the Always on Top pin). The other tokens are skipped silently: the host's own `AppMenuButton` goes in the `start` slot.
- Order is preserved exactly per side. Each non-empty side renders one `data-wp-controls` group with `role="group"`, so there can be two. An empty side renders nothing, and two empty sides render no controls.
- The pin is drawn only when `showAlwaysOnTop` is set. If the layout has no `keepAbove`, it goes immediately before the first `minimise`, `maximise` or `close` on the end side (or at the end of that side when none is there). If the layout has `keepAbove`, it goes at that position.
- The `win11` style is flush with the window edge: the negative margin applies to the end group and mirrors on the start group.

### Titlebar actions

`titlebarActions` is `{ doubleClick, middleClick, rightClick }`, each a `TitlebarAction`: `toggleMaximise`, `toggleMaximiseHorizontally`, `toggleMaximiseVertically`, `toggleShade`, `minimise`, `lower`, `menu` or `none`. The default, `DEFAULT_TITLEBAR_ACTIONS`, is double-click `toggleMaximise`, middle-click `none` and right-click `menu`. Only empty bar space triggers actions: buttons, links, inputs, and anything marked `data-window-menu-exclude` (the app menu button) are excluded.

- `toggleMaximise` calls `toggleMaximize()` (skipped when the adapter sets `handlesDoubleClickNatively`), `minimise` calls `minimize()`, and `menu` opens the window menu (Restore or Maximise, Minimise, Move, Always on Top, Close). The other actions a desktop can be set to (`toggleShade`, `lower` and the two directional maximise actions) have no adapter method, so `performedAction()` treats them as `none`: the gesture does nothing and, for a right-click, the platform's own behaviour is left alone. `PERFORMED_ACTIONS` lists what the chrome performs.
- Double-click runs `doubleClick`, middle-click (`auxclick` button 1) runs `middleClick`, and right-click runs `rightClick`. For right-click only `menu` acts and suppresses the native context menu; any other value does nothing and leaves the default alone.
- Limitation: when the adapter has `handlesDoubleClickNatively` (Tauri's drag-region script maximises on double-click) and `doubleClick` is not `toggleMaximise`, the chrome cannot suppress the native maximise. The configured action runs in addition to it. Use a host adapter without native handling if the double-click action must differ from maximise.

## Window frame

`WindowFrame` is the outermost element of a frameless, transparent window. It clips its surface to `--wp-window-radius`, draws a hairline border, and draws `--wp-window-shadow` inside a transparent margin of `--wp-window-shadow-margin` for platforms where the OS paints none. While the window is unfocused it switches to `--wp-window-shadow-unfocused`. A maximised window has no margin, shadow, rounding or border. `className` applies to the visible surface, not the margin. It reads window state from `WindowChromeProvider`.

## Focus

The provider follows window focus through the adapter's optional `isFocused` and `onFocusChange`. An unfocused title bar dims its text and controls (`--wp-text-muted`, and `--wp-bg-chrome-unfocused` if set), and an unfocused frame lightens its shadow. A host that cannot report focus is treated as always focused. The context menu dismisses when the window goes from focused to unfocused, never merely because the window was already unfocused when the menu opened; without a provider it falls back to the DOM `blur` event.

## Context menu

`ContextMenu` is controlled: pass `items`, a viewport `position`, `onSelect` and `onClose`. Item types are `action` (with an optional `danger` flag), `checkbox`, `submenu`, `separator` and `section`. It follows the WAI-ARIA menu pattern — `role="menu"`, `menuitem` and `menuitemcheckbox`, arrow keys, `Home` and `End`, `Enter` and `Space`, `Esc`, type-ahead, submenus opened by `ArrowRight` or after 150 ms of hover — clamps itself to the viewport, and returns focus to the element that was focused when it opened (or to `returnFocusTo`). It closes on a click outside, on resize, and on window focus loss as described under Focus.

## Tokens

Styling uses CSS custom properties with fallbacks, so the package renders before an app defines them: `--wp-bg-raised`, `--wp-bg-chrome`, `--wp-border-subtle`, `--wp-text-primary`, `--wp-text-muted`, `--wp-accent`, `--wp-accent-contrast`, `--wp-danger`, `--wp-focus-ring`, `--wp-control-bg`, `--wp-control-bg-hover`, `--wp-title-bar-height`, `--wp-title-bar-opacity`, `--wp-window-radius`, `--wp-window-shadow`, `--wp-window-shadow-unfocused`, `--wp-window-shadow-margin`, `--wp-bg-chrome-unfocused`, `--wp-font-mono`, `--wp-shadow-menu` and `--wp-z-menu`.

## Localisation

Components take a `labels` prop (`ChromeLabels`) and default to English strings from `defaultChromeLabels`; the app supplies translated values from its message catalogue.
