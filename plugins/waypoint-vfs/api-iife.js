if ('__TAURI__' in window) {
var __TAURI_PLUGIN_WAYPOINT_VFS__ = (function (exports, core, webviewWindow) {
    'use strict';

    // Exposes typed guest-side wrappers for the waypoint-vfs plugin
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PREFIX = 'plugin:waypoint-vfs|';
    const LISTING_EVENT = 'waypoint-vfs://listing';
    function cmd(name, args) {
        return core.invoke(`${PREFIX}${name}`, args);
    }
    /**
     * Reports whether the file system plugin works here, and which features: `listing`, `watch`,
     * `places`, `trash-view` when the Trash can be browsed, and `polling-fallback` while a listing is
     * kept up to date by polling.
     */
    function getStatus() {
        return cmd('get_status');
    }
    /**
     * Opens a listing of a folder and resolves with its first snapshot (phase `scanning`). The scan
     * continues in Rust and is reported through `onListingEvent`. Rejects with a `VfsError`.
     */
    function openListing(location, options = {}) {
        return cmd('open_listing', { location, options });
    }
    /** Reads `count` entries from view position `start`; shorter at the end of the listing. */
    function getRange(handle, start, count) {
        return cmd('get_range', { handle, start, count });
    }
    /** Re-sorts a listing and resolves with its new snapshot. Cached pages are stale afterwards. */
    function setSort(handle, sort) {
        return cmd('set_sort', { handle, sort });
    }
    /** Changes what a listing hides and resolves with its new snapshot. Cached pages are stale. */
    function setFilter(handle, filter) {
        return cmd('set_filter', { handle, filter });
    }
    /** Closes a listing, cancelling a scan in flight. Closing an unknown handle is not an error. */
    function closeListing(handle) {
        return cmd('close_listing', { handle });
    }
    /** The folder a window opens at first. */
    function getHome() {
        return cmd('get_home');
    }
    /**
     * Turns text the person typed (an absolute or relative path, `~`, a `file://` URI) into a
     * `Location`, resolving relative text against `base`. Rejects with `invalidLocation` or
     * `unsupported` (another scheme); it does not check that the location exists.
     */
    function parseLocation(input, base) {
        return cmd('parse_location', { input, base });
    }
    /** The parent and the breadcrumb segments of a location. */
    function describeLocation(location) {
        return cmd('describe_location', { location });
    }
    /** Where an entry of an open listing lives; `.display` is what Copy Path uses. */
    function entryLocation(handle, id) {
        return cmd('entry_location', { handle, id });
    }
    /** The count and total file size of a selection over a listing's current view. */
    function summariseSelection(handle, selection) {
        return cmd('summarise_selection', { handle, selection });
    }
    /** Free and total space on the volume holding `location`, or `null` where it cannot be known. */
    function getFreeSpace(location) {
        return cmd('get_free_space', { location });
    }
    /**
     * Opens a file of an open listing in its default application. Rust resolves the path from
     * `(handle, id)`; a folder is rejected with `unsupported`.
     */
    function openEntry(handle, id) {
        return cmd('open_entry', { handle, id });
    }
    /**
     * Whether the Trash can be browsed here, why not, and how many items it holds. Reading it lists the
     * Trash, so ask when the number is wanted (the sidebar does, on a slow timer and on focus).
     */
    function getTrashInfo() {
        return cmd('get_trash_info');
    }
    /** Home, the user folders that exist, and the favourites. */
    function listPlaces() {
        return cmd('list_places');
    }
    /** Pins a folder to the favourites, with an optional label. Resolves with the updated places. */
    function addFavourite(location, label) {
        return cmd('add_favourite', { location, label: label ?? null });
    }
    /** Unpins a folder. Resolves with the updated places. */
    function removeFavourite(location) {
        return cmd('remove_favourite', { location });
    }
    /** Labels a favourite, or clears its label with `null`. Resolves with the updated places. */
    function renameFavourite(location, label) {
        return cmd('rename_favourite', { location, label });
    }
    /** Moves a favourite to position `to` in the list. Resolves with the updated places. */
    function moveFavourite(location, to) {
        return cmd('move_favourite', { location, to });
    }
    /**
     * Listens for progress, live patches and failures of every listing the calling window opened.
     * Subscribe first and then open listings, so no event between the two is missed. Resolves once
     * the listener is registered; call the returned function to stop listening.
     */
    function onListingEvent(handler) {
        return webviewWindow.getCurrentWebviewWindow().listen(LISTING_EVENT, (event) => handler(event.payload));
    }

    exports.addFavourite = addFavourite;
    exports.closeListing = closeListing;
    exports.describeLocation = describeLocation;
    exports.entryLocation = entryLocation;
    exports.getFreeSpace = getFreeSpace;
    exports.getHome = getHome;
    exports.getRange = getRange;
    exports.getStatus = getStatus;
    exports.getTrashInfo = getTrashInfo;
    exports.listPlaces = listPlaces;
    exports.moveFavourite = moveFavourite;
    exports.onListingEvent = onListingEvent;
    exports.openEntry = openEntry;
    exports.openListing = openListing;
    exports.parseLocation = parseLocation;
    exports.removeFavourite = removeFavourite;
    exports.renameFavourite = renameFavourite;
    exports.setFilter = setFilter;
    exports.setSort = setSort;
    exports.summariseSelection = summariseSelection;

    return exports;

})({}, __TAURI__.core, __TAURI__.webviewWindow);
Object.defineProperty(window.__TAURI__, 'waypointVfs', { value: __TAURI_PLUGIN_WAYPOINT_VFS__ }) }
