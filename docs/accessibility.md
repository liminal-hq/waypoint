# Accessibility and Focus Review

Status: prototype review, 2026-09-29. Findings are from reading the markup and behaviour, not from testing with a screen reader yet.

## What is in place

- **Landmarks:** the tab strip is a tablist (each tab has aria-selected), the toolbar is a toolbar, the sidebar is a navigation region, the file area is the main region, and the status bar is a polite live region.
- **Keyboard:** every action has a keyboard path (see the shortcut list, press ?). Shortcuts can be rebound in Settings → Keyboard, with conflict detection.
- **Focus:** a setting turns on a thick, always-visible focus ring.
- **Motion:** a setting turns off all animation and transitions, including drag feedback.
- **Contrast:** a high-contrast setting strengthens borders and lifts muted text. Transparency keeps a solid backing behind text.
- **Touch:** touch mode raises rows, buttons and menu items to at least 44 pixels; Auto turns it on in narrow windows.
- **Text size:** Default, Large (115%), Larger (130%).
- **Direction:** right-to-left mirrors the window, tabs, sidebar and back/forward arrows.

## Still to do (found in this review)

1. **File rows need roles.** Rows are plain elements. Give the list role=listbox with aria-multiselectable, each row role=option with aria-selected, and announce the selection count.
2. **Menus.** Context menus need role=menu and menuitem, arrow-key focus, and to return focus to the trigger on close.
3. **Dialogs.** Give them role=dialog with a label, trap focus, and restore it on close. Escape already closes them.
   - The shared `Dialog` (`docs/shared-components.md`) gives role=dialog (native) named by its title, focus into the dialog on open (never on a destructive button), a Tab and Shift+Tab trap, Esc, focus restored to the opener, and a non-dismissible mode for questions that must be answered.
   - Checklist for each dialog the app builds on it: the title says what the dialog is about; destructive confirmations use `ConfirmDialog` with `danger`; a dialog that cannot be dismissed offers a visible way out or an answer to every question; stacked dialogs return focus to the one beneath.
4. **Drag and drop.** Provide a non-pointer path for every drop: Copy To… and Move To… cover files; the tab menu covers splitting. Announce drop targets and results in the live region.
5. **Progress.** Done for the queue (milestone 4, slice 06): the status bar ring is a button named by its state ("Operations, 2 in progress, 50% complete, waiting for you"), with `aria-expanded` and `aria-haspopup="dialog"`, and colour is never the only signal (the count, the label and a dot with the state in the name). The popover is a non-modal `role=dialog` named "Operations": it opens with Enter or Space, puts focus on the first job row (or on the panel when the list is empty), closes with Escape and returns focus to the ring, and closes without taking focus when a press or Tab lands outside. The rows are a list with one tab stop (Up, Down, Home and End move between rows; Alt+Up and Alt+Down reorder a queued job and announce the new position); each row is named by its title and described by its state, and its buttons are named "Action: title". The live region speaks a job starting (once), waiting for the person, failing, being cancelled and finishing, with how many others are still in progress, and 25, 50 and 75 % of a job that has run three seconds, never every tick. The ring does not animate (a job not yet sized is a still quarter, not a spinner) and its fill transition is off under reduced motion. Still to do: a screen reader pass, and the Operations window's own focus order.
6. **Focus order.** Title bar, tab strip, toolbar, action bar, sidebar, files, inspector, status bar. Confirm with a keyboard-only pass.
7. **Colour is never the only signal.** Git status has letters; tags need names in the row tooltip; connection dots need text alternatives.
8. **Icon-only buttons** (view switcher, joint, action bar in narrow windows) need accessible names. Most have a title; add aria-labels.
9. **Pointer-only gestures** (long-press history, hold-to-split) need keyboard equivalents. Alt+Left/Right with a menu key, and Split With in the tab menu, cover them.
10. **Minimum sizes:** text stays at or above 12 px; touch mode covers 44 px targets.
