# Shared Components (Liminal family)

## Unified title bar

The copies have drifted. Spindle, Cadence, Threshold and Jar each have their own title bar. Waypoint defines a single standard that should be pushed back into the others.

| Capability                                                         | Source    | Unified behaviour                                                                |
| ------------------------------------------------------------------ | --------- | -------------------------------------------------------------------------------- |
| App mark and menu button, with the menu openable from the keyboard | Jar       | Left side. Alt or F10 opens the menu.                                            |
| Drag region on the bar and its non-interactive children            | Jar       | `data-tauri-drag-region` on all passive nodes                                    |
| Maximised-state hook (changes the restore glyph and radius)        | Jar       | Square corners when maximised                                                    |
| Always on Top toggle                                               | Threshold | Optional slot, on in Waypoint                                                    |
| App content in the bar (search, title, status)                     | Spindle   | Centre slot. Waypoint shows the location title only. Tabs stay in their own row. |
| Double-click to maximise                                           | All       | Standard                                                                         |
| Platform control layout                                            | New       | GNOME/KDE button layout from the OS, Win11 caption style, macOS-style unused     |
| Transparency-aware                                                 | New       | Follows the Transparency "Title bar" region                                      |

Slots: `start` (mark and menu) · `center` (title and app content) · `end` (app actions) · `controls` (window buttons).

The window's title is one value for every place it shows: `useWindowTitle(title)` sets the centre text (`<TitleBarTitle fallback="…" />`), the host's title and `document.title` together, and the provider's `formatTitle` shapes only the centre text (D197).

`AppMenuButton` takes `items` (a menu with submenus), `onSelect`, `acceleratorKeys` (F10 and a lone Alt, on by default) and `mnemonics`, a map from a lower-case letter to the id of a top-level submenu item: Alt plus the letter opens the menu with that submenu open and its first row focused (Alt with Ctrl, Shift or Meta is left alone). The `start` slot sits after any window buttons the layout puts on the start side, so the menu never collides with the controls on either side; pressing the button while its menu is open closes it. Waypoint's `AppMenu` builds the items from the command registry (`apps/waypoint/src/commands/`), so rows are hidden, disabled with a reason as the tooltip, or checked from the window's state.

## Context menu

Source: `ScottMorris/liminal-notes` (Editor ContextMenu) and `liminal-hq/jar` (ContextMenu module).

- Item types: action, checkbox, submenu, separator, section label, danger.
- Anatomy: a 16 px icon, a label, a right-aligned shortcut in a muted monospace font, and a submenu chevron.
- Icons: every action, checkbox and submenu item has an icon, so labels line up and the menu reads the same everywhere. Colour items show their swatch, and a checked checkbox shows the check mark in place of its icon. Icons are `aria-hidden` (the label always carries the meaning) and take the secondary text colour, or the row's own colour when it is highlighted or a danger item.
- Where icons live: chrome menus use `packages/chrome/src/icons/icons.tsx`; the app's menus use `apps/waypoint/src/icons/MenuIcons.tsx` (and `AppIcons.tsx`). The item-to-icon mapping sits in each menu's item builder, and each builder's test calls `expectEveryItemHasIcon` (`@liminal-hq/waypoint-chrome/ContextMenu/expectEveryItemHasIcon`), which walks nested submenus, so a new item cannot ship without one.
- Behaviour: positioned to stay in the viewport, arrow keys, Enter and Esc work, typing a letter jumps to an item, submenus open on hover after 150 ms.
- Styling: a raised surface, a subtle border, a 8 px radius, 4 px padding, rows 28 px high and 6 px row radius, accent hover.

## Settings shell

Same as Emoji Nook's SettingsPanel: a side-nav of sections with grouped rows (label, description, control), so every Liminal app shares one settings pattern.

Both components are controlled, own no persistence, import nothing from Waypoint, and read only `--wp-*` tokens (defaults in `packages/chrome/src/tokens.css`). Import each from its own file, for example `@liminal-hq/waypoint-chrome/SettingsShell/SettingsShell`.

### SettingsShell

`SettingsShell` takes `sections: SettingsSectionDef[]` (`{ id, label, icon?, render }`, from `SettingsShell/types`), a controlled `activeId` with `onSelect(id)`, an optional `collapseBelow` width in pixels (default 640), a `toolbar` slot above the page heading (reserved for a search box, nothing is built into it), and partial `labels` for translation. Only the active section's `render()` is called, and an unknown `activeId` falls back to the first section.

- Nav: a `nav` of buttons with an optional 16 px icon, `aria-current="page"` on the active one and a roving tab stop. Up, Down, Home and End move focus (Down and Up wrap); Enter and Space activate, so arrowing through the list does not swap pages under the reader.
- Collapse: a `ResizeObserver` on the shell turns the nav into a select above the page when the shell is narrower than `collapseBelow`. Without a `ResizeObserver` the nav stays expanded. The width is a prop because CSS custom properties cannot be used in a size query.
- Page: a `main` named by its `h2` heading; groups use `h3`.
- Sizing: controls are `--wp-control-height` high (28 px by default). A host raises that token to 44 px for touch mode, and rows grow with it. Rows use logical properties, so right-to-left mirrors without extra rules.

`SettingsSection` wraps a page body (optional `description`, then groups). `SettingsGroup` is a titled `role="group"` card of rows (`title`, optional `description`).

Every row is a `SettingsRow`: `label`, `description?`, `disabled?`, `unavailableReason?` and a control slot. The row wires the label, description and reason line to its control by id (`useSettingsRowControl()` hands a custom control the same ids). `disabled` dims the row and disables the control. `unavailableReason` does the same and adds an "Unavailable: …" line, which is the Services-panel convention for an option this system cannot offer; an app that hides such options simply omits the row.

| Row            | Control                                     | Props beyond the row's own                                                         |
| -------------- | ------------------------------------------- | ---------------------------------------------------------------------------------- |
| `ToggleRow`    | a switch (`role="switch"`, `aria-checked`)  | `checked`, `onChange(checked)`                                                     |
| `SelectRow`    | a native select                             | `value`, `options: { value, label, disabled? }[]`, `onChange(value)`               |
| `NumberRow`    | a number field with an optional unit suffix | `value`, `onChange(n)`, `min`, `max`, `step`, `unit`                               |
| `SegmentedRow` | a radio group of buttons with arrow keys    | `value`, `options: { value, label, disabled? }[]`, `onChange(value)`               |
| `ButtonRow`    | one action button (the label describes it)  | `actionLabel`, `onAction`, `danger?`                                               |
| `LinkRow`      | the whole row is a link with a chevron      | `href`, `onActivate?` (cancels navigation so the app can open it), no control slot |

`NumberRow` reports in-range edits as they are typed and clamps anything else when the field loses focus or Enter is pressed; an empty field reverts. With `commitOn="commit"` it reports nothing until the field is left or Enter is pressed, for a setting that must not take the half-typed values on the way to the one meant. Any row takes `error`: the app's sentence for a refused change, shown under the description as an `alert` that the control's accessible description includes, with number and select fields marked `aria-invalid` while it shows.

### Dialog

`Dialog` is a controlled modal over the native `<dialog>` element, opened with `showModal()`, so the top layer, the inert page and Esc come from the platform. It renders nothing while `open` is false.

| Prop            | Meaning                                                                                              |
| --------------- | ---------------------------------------------------------------------------------------------------- |
| `open`          | Whether the dialog is shown. The app owns it; nothing closes until the app sets it false.            |
| `onClose`       | `(reason: 'escape' \| 'cancel' \| 'backdrop' \| 'action') => void`, asked for, never forced.         |
| `title`         | The heading and the accessible name (`aria-labelledby`).                                             |
| `description`   | Optional supporting text, wired up as `aria-describedby`.                                            |
| `size`          | `small`, `medium` (default) or `large`, from the `--wp-dialog-width-*` tokens.                       |
| `footer`        | The button row, usually `DialogActions`.                                                             |
| `initialFocus`  | A ref or a selector inside the dialog.                                                               |
| `dismissible`   | Default true. False makes Esc and a backdrop click do nothing, for a question that must be answered. |
| `returnFocusTo` | Where focus goes on close; defaults to the element focused at open.                                  |

- Default focus: the first field in the body, otherwise a footer button that is never a danger button (Cancel when a danger button exists, otherwise the primary one). With only a danger button the dialog surface takes focus.
- Dismissal: `escape` for Esc, `cancel` for a native cancel request that was not the Esc key, `backdrop` only when both the press and the release land on the backdrop, so dragging a text selection out of a field does not close the dialog. `action` comes from a `DialogButton` with `closes`.
- Stacking: a dialog rendered inside another is in front; only the front one answers Esc and Tab, and closing it returns focus to where it came from.
- Scroll lock: a reference-counted lock on the root element that pads by the scrollbar width, so the page does not shift.
- Motion: a short fade and rise, off under `prefers-reduced-motion`.
- Transparency: the surface is `--wp-bg-raised` mixed with transparent by `--wp-dialog-opacity` (1 by default), the same hook the other surfaces use.
- Fallback: when `showModal` is missing or throws, the dialog opens with the `open` attribute, draws its own scrim and keeps its own Tab trap. The Tab trap also runs on the native path, where it agrees with the platform.
- No live regions: the native dialog role and name are the announcement; an app that wants more announces through its own live-region store.

`DialogActions` lays out footer buttons at the end edge. `DialogButton` takes `variant` (`primary`, `secondary` default, `danger`) and `closes`. `ConfirmDialog` takes `open`, `title`, `message`, `confirmLabel`, `cancelLabel` (default "Cancel"), `danger`, `size` (default small), `onConfirm` and `onCancel`; Esc and the backdrop call `onCancel`, and focus starts on Cancel when `danger` is set.
