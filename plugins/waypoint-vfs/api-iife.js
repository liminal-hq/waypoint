if ('__TAURI__' in window) {
var __TAURI_PLUGIN_WAYPOINT_VFS__ = (function (exports, core, event, webviewWindow) {
    'use strict';

    // Exposes typed guest-side wrappers for the waypoint-vfs plugin
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PREFIX = 'plugin:waypoint-vfs|';
    const LISTING_EVENT = 'waypoint-vfs://listing';
    const CONNECTIONS_EVENT = 'waypoint-vfs://connections';
    const CONNECTION_STATE_EVENT = 'waypoint-vfs://connection-state';
    function cmd(name, args) {
        return core.invoke(`${PREFIX}${name}`, args);
    }
    /**
     * Reports whether the file system plugin works here, and which features: `listing`, `watch`,
     * `places`, `entry-details`, `folder-size`, `text-head`, `preview-protocol`, `trash-view` when the Trash can be browsed, and `polling-fallback` while a listing is
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
     * Whether `location` is a folder and can be written to, for a destination picker. Rejects with
     * `notFound` (or `permissionDenied`) where it cannot be seen at all.
     */
    function checkFolder(location) {
        return cmd('check_folder', { location });
    }
    /**
     * Opens a file of an open listing in its default application. Rust resolves the path from
     * `(handle, id)`; a folder is rejected with `unsupported`.
     */
    function openEntry(handle, id) {
        return cmd('open_entry', { handle, id });
    }
    /**
     * Everything the Inspector shows about one entry of an open listing: kind, exact and allocated
     * size, times, owner and group, permissions, symlink target, hidden flag and content type. A field
     * the provider cannot report is named in `unavailable`; a field the entry does not have is `null`.
     */
    function entryDetails(handle, id) {
        return cmd('entry_details', { handle, id });
    }
    /**
     * Starts totalling a folder of an open listing and resolves with the run as soon as it has
     * started. `onEvent` gets `progress` about every 100 ms and then exactly one `done`, `cancelled`
     * or `failed`. The walk is low priority, stays on one volume, never follows a symlink and never
     * downloads a cloud placeholder. Rejects (`notADirectory`) for an entry that is not a folder.
     */
    async function folderSize(handle, id, onEvent) {
        const channel = new core.Channel();
        channel.onmessage = onEvent;
        const job = await cmd('folder_size', { handle, id, onEvent: channel });
        return { job, cancel: () => cancelFolderSize(job) };
    }
    /** Stops a folder-size run of this window. A run that has ended is not an error. */
    function cancelFolderSize(job) {
        return cmd('cancel_folder_size', { job });
    }
    /**
     * Starts scanning the top-level folders of `location` for their sizes and resolves with the run as
     * soon as it has started. `onEvent` gets `progress` about every 100 ms, a `partial` result after
     * each top-level folder (every row so far, with its share of what has been scanned, and a
     * remainder row for loose files and hidden items), and then exactly one `done`, `cancelled` or
     * `failed`. The scan is low priority on a thread of its own, stays on one volume, never follows a
     * symlink and never downloads a cloud placeholder. A finished scan is cached for
     * `getCachedDirScan`.
     */
    async function scanDirSizes(location, onEvent, options) {
        const channel = new core.Channel();
        channel.onmessage = onEvent;
        const job = await cmd('scan_dir_sizes', { location, options, onEvent: channel });
        return { job, cancel: () => cancelDirScan(job) };
    }
    /** Stops a directory-size scan of this window. A scan that has ended is not an error. */
    function cancelDirScan(job) {
        return cmd('cancel_dir_scan', { job });
    }
    /**
     * The last finished scan of `location`, with `measuredAtMs` for "as of <time>", or `null` when
     * there is none.
     */
    function getCachedDirScan(location) {
        return cmd('get_cached_dir_scan', { location });
    }
    /**
     * The first bytes of a file of an open listing as text: at most `max` bytes (default and ceiling
     * 256 KiB), decoded as UTF-8 with invalid sequences replaced. Rejects with `notText` for a binary
     * file and `isADirectory` for a folder.
     */
    function readTextHead(handle, id, max) {
        return cmd('read_text_head', { handle, id, max: max ?? null });
    }
    /** The custom scheme that serves entries to this window. */
    const PREVIEW_SCHEME = 'wpfile';
    /**
     * The URL that serves an entry's bytes through the `wpfile` protocol, for an `<img>`, `<audio>`,
     * `<video>` or `fetch`, with `Range` support. It is a token over this window's own listings, never
     * a path: another window's URL, a closed listing and an entry that has gone all answer 404.
     */
    function previewUrl(handle, id) {
        return core.convertFileSrc(`${handle}-${id}`, PREVIEW_SCHEME);
    }
    /**
     * Whether the Trash can be browsed here, why not, and how many items it holds. Reading it lists the
     * Trash, so ask when the number is wanted (the sidebar does, on a slow timer and on focus).
     * `withBytes` also adds up the sizes into `totalBytes`; ask only while Overview is visible.
     */
    function getTrashInfo(withBytes = false) {
        return cmd('get_trash_info', { withBytes });
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
    /** The saved connections and recent servers, with the state of every login Rust knows. */
    function listConnections() {
        return cmd('list_connections');
    }
    /**
     * The server protocols a provider serves here, and why a login cannot be remembered in the keyring
     * (`null` when it can). Never asks the keyring to unlock.
     */
    function connectionSupport() {
        return cmd('connection_support');
    }
    /** Hosts of `~/.ssh/config` to offer in the Connect dialog. */
    function suggestedServers() {
        return cmd('suggested_servers');
    }
    /**
     * Reads a typed server address into the dialog's fields, saying whether a password written in it
     * was dropped. Rejects with a `VfsError` (`invalidLocation`, or `unsupported` for a protocol no
     * provider serves).
     */
    function parseAddress(text) {
        return cmd('parse_address_text', { text });
    }
    /** Saves a new connection. Rejects with `{ kind: 'connections', error }` naming the bad field. */
    function addConnection(draft) {
        return cmd('add_connection', { draft });
    }
    /** Changes a saved connection. */
    function updateConnection(id, draft) {
        return cmd('update_connection', { id, draft });
    }
    /** Saves a copy of a connection right after it, under `name`. */
    function duplicateConnection(id, name) {
        return cmd('duplicate_connection', { id, name });
    }
    /**
     * Forgets a saved connection; with `forgetLogin` its remembered secrets go too. Resolves with why
     * the keyring could not forget them, or `null`.
     */
    function removeConnection(id, forgetLogin) {
        return cmd('remove_connection', { id, forgetLogin });
    }
    /** Moves a saved connection to position `to`. */
    function moveConnection(id, to) {
        return cmd('move_connection', { id, to });
    }
    /** Forgets one recent server by its login, or all of them with `null`. */
    function forgetRecentServer(key) {
        return cmd('forget_recent_server', { key });
    }
    /** Forgets the remembered secrets of a server's login. Resolves with why it could not, or `null`. */
    function forgetLogin(location) {
        return cmd('forget_login', { location });
    }
    /**
     * Connects a server's login now, with the person's answer to the question its last attempt asked
     * (none retries, as Reconnect does). The answer, which may hold a secret, is sent once and never
     * comes back. Rejects with the `VfsError` that says what is still needed.
     */
    function connect(location, answer = null, remember = false) {
        return cmd('connect', { location, answer, remember });
    }
    /** Tries a draft's server without saving it, as `connect` does. */
    function testConnection(draft, answer = null, remember = false) {
        return cmd('test_connection', { draft, answer, remember });
    }
    /** Closes a server's login; its listings show the disconnected state. */
    function disconnect(location) {
        return cmd('disconnect', { location });
    }
    /** The state of the login a location belongs to, or `null` for one with no login. */
    function connectionState(location) {
        return cmd('connection_state', { location });
    }
    /** Hears every change to the saved connections and recent servers, in any window. */
    function onConnectionsChanged(handler) {
        return event.listen(CONNECTIONS_EVENT, (event) => handler(event.payload));
    }
    /** Hears every change of a login's state. */
    function onConnectionState(handler) {
        return event.listen(CONNECTION_STATE_EVENT, (event) => handler(event.payload));
    }

    exports.PREVIEW_SCHEME = PREVIEW_SCHEME;
    exports.addConnection = addConnection;
    exports.addFavourite = addFavourite;
    exports.cancelDirScan = cancelDirScan;
    exports.cancelFolderSize = cancelFolderSize;
    exports.checkFolder = checkFolder;
    exports.closeListing = closeListing;
    exports.connect = connect;
    exports.connectionState = connectionState;
    exports.connectionSupport = connectionSupport;
    exports.describeLocation = describeLocation;
    exports.disconnect = disconnect;
    exports.duplicateConnection = duplicateConnection;
    exports.entryDetails = entryDetails;
    exports.entryLocation = entryLocation;
    exports.folderSize = folderSize;
    exports.forgetLogin = forgetLogin;
    exports.forgetRecentServer = forgetRecentServer;
    exports.getCachedDirScan = getCachedDirScan;
    exports.getFreeSpace = getFreeSpace;
    exports.getHome = getHome;
    exports.getRange = getRange;
    exports.getStatus = getStatus;
    exports.getTrashInfo = getTrashInfo;
    exports.listConnections = listConnections;
    exports.listPlaces = listPlaces;
    exports.moveConnection = moveConnection;
    exports.moveFavourite = moveFavourite;
    exports.onConnectionState = onConnectionState;
    exports.onConnectionsChanged = onConnectionsChanged;
    exports.onListingEvent = onListingEvent;
    exports.openEntry = openEntry;
    exports.openListing = openListing;
    exports.parseAddress = parseAddress;
    exports.parseLocation = parseLocation;
    exports.previewUrl = previewUrl;
    exports.readTextHead = readTextHead;
    exports.removeConnection = removeConnection;
    exports.removeFavourite = removeFavourite;
    exports.renameFavourite = renameFavourite;
    exports.scanDirSizes = scanDirSizes;
    exports.setFilter = setFilter;
    exports.setSort = setSort;
    exports.suggestedServers = suggestedServers;
    exports.summariseSelection = summariseSelection;
    exports.testConnection = testConnection;
    exports.updateConnection = updateConnection;

    return exports;

})({}, __TAURI__.core, __TAURI__.event, __TAURI__.webviewWindow);
Object.defineProperty(window.__TAURI__, 'waypointVfs', { value: __TAURI_PLUGIN_WAYPOINT_VFS__ }) }
