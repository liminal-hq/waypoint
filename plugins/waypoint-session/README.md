# tauri-plugin-waypoint-session

Waypoint's session plugin: it holds each window's tabs, their order, the active tab and per-tab back and forward history over the `waypoint-session` crate, and tells the window about every change (A20).

This is a domain plugin, private to Waypoint (see `docs/architecture/crates-and-plugins.md`). Its JavaScript API is the `@liminal-hq/waypoint-plugin-session` package in `guest-js/`; the app reaches the plugin only through it, and its wire types come from `@liminal-hq/waypoint-protocol`.

## How it works

- **One session per window**, keyed by the window label and created on first use. It is dropped when the window is destroyed.
- **Commands** are `get_snapshot`, `open_tab`, `close_tab`, `activate_tab`, `move_tab`, `navigate`, `back`, `forward` and `get_status`. A command applies to the calling window's session.
- **Events** arrive on `waypoint-session://event`, one per granular change (`tabOpened`, `tabClosed`, `tabActivated`, `tabMoved`, `tabNavigated`), each with the new revision. `onTabsEvent` subscribes; read `getSnapshot()` first and apply events with a higher revision.
- **Tab-close hook.** The composition root calls `app.state::<Sessions>().on_tab_closed(|window_label, tab| …)` to release what a tab held, such as its listing handle. It also fires for every remaining tab when a window closes.

## Status

Tabs, order, the active tab and history. Groups, pairs, pinning and persistence arrive in milestone 3.
