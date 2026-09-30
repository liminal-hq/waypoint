# tauri-plugin-waypoint-vfs

Waypoint's file system plugin: it opens listings over the `waypoint-vfs` crate and serves them to the frontend by range, so the webview holds only the rows near the viewport (A9). It also owns places and favourites.

This is a domain plugin, private to Waypoint (see `docs/architecture/crates-and-plugins.md`). Its JavaScript API is the `@liminal-hq/waypoint-plugin-vfs` package in `guest-js/`; the app reaches the plugin only through it, and its wire types come from `@liminal-hq/waypoint-protocol`.

## Status

A skeleton. It registers and reports itself unavailable through `getStatus()`; the listing, sort, filter, range and places commands arrive in later slices of milestone 2.
