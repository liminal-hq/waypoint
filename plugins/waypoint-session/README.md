# tauri-plugin-waypoint-session

Waypoint's session plugin: it holds every window's tabs, groups, pairs, pins, colours, view and geometry as one `Store` over the `waypoint-session` crate, and tells each window about every change to it (A20, A37).

This is a domain plugin, private to Waypoint (see `docs/architecture/crates-and-plugins.md`). Its JavaScript API is the `@liminal-hq/waypoint-plugin-session` package in `guest-js/`; the app reaches the plugin only through it, and its wire types come from `@liminal-hq/waypoint-protocol`.

## How it works

- **One store, one lock.** `Sessions<R>` (Tauri state) holds the `Store` behind one `Mutex`. A command locks it, applies one `Command`, creates any window the command made, delivers the events and fires the hooks before unlocking, so a window receives its events in revision order.
- **Commands act on the calling window** (its label), unless they name a target (`close_window`'s `target`, `move_tabs`' `to`). They are `get_snapshot`; `open_tab`, `close_tab`, `activate_tab`, `move_tab`, `navigate`, `back`, `forward`, `pin_tab`, `set_tab_colour`, `set_tab_hints`, `reopen_tab`; `create_group`, `add_to_group`, `remove_from_group`, `rename_group`, `set_group_colour`, `set_group_collapsed`, `collapse_other_groups`, `sort_group`, `duplicate_group`, `move_group`, `ungroup`, `close_group`; `join_pair`, `separate_pair`, `set_pair_layout`, `set_pair_sizes`, `swap_panes`, `toggle_split`; `open_window`, `close_window`, `set_geometry`, `set_view`, `move_tabs`; and `get_status`.
- **Events** arrive on `waypoint-session://event`, each sent to the window it belongs to only (`emit_to`) and carrying the store's global revision, so a window sees gaps. `onTabsEvent` subscribes; read `getSnapshot()` first and apply events with a higher revision. A window that does not exist yet (the target of a hand-off) gets its state from its snapshot when it starts; a failed send is logged and never fails the command.
- **First run.** A main window (`main-{n}`) the store does not hold yet is registered, empty, under its own label by its first `get_snapshot` or command (`Command::RegisterWindow`), so the page just reads its snapshot and opens a tab. A label that is not a main window is refused.
- **Injected dependencies.** `init(SessionDeps { create_window, storage, policy, on_last_window_closed, on_change, change_delay })`:
  - `create_window: Arc<dyn WindowFactory<R>>` builds the webview for a window a command made (`open_window`, `move_tabs` to a new window). The plugin calls it after the store change and before any event is sent, with the store locked, so it must not call back into the session commands; if it fails, the store goes back to how it was and the command fails.
  - `storage: Arc<dyn SessionStorage>` is held for the app's persistence wiring (`Sessions::storage`); the plugin does not load or save by itself yet.
  - `policy` is the store's `StorePolicy` (`close_window_on_last_tab`).
  - `on_last_window_closed` runs when the last window of the store closes, outside the lock.
  - `on_change` is called with a copy of the store `change_delay` (default one second) after the first change of a burst, once per burst, from a helper thread. It is the hook persistence subscribes to.
- **Tab-close hook.** The composition root calls `app.state::<Sessions<Wry>>().on_tab_closed(|window_label, tab| …)` to release what a tab held, such as its listing handle. It fires for a tab that is closed or leaves with its window, not for one handed to another window.
- **Window lifecycle.** `WindowEvent::Destroyed` closes the window's session (its tabs go to the closed list). It runs off the main thread, because a window factory holds the lock while it waits for the main thread. `close_window` closes the session and then destroys the webview.

## Status

Tabs, order, the active tab, history, pins, colours, hints, reopen, groups, pairs, windows, geometry, view and hand-off, in memory. Persistence and window creation from Rust arrive in slices 03 and 04 of milestone 3.
