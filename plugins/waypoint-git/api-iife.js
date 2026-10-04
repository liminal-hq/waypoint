if ('__TAURI__' in window) {
var __TAURI_PLUGIN_WAYPOINT_GIT__ = (function (exports, core, event) {
    'use strict';

    // Exposes typed guest-side wrappers for the waypoint-git plugin
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PREFIX = 'plugin:waypoint-git|';
    /** Sent to the window that watches a repository when its state changes; the payload is a `GitChanged`. */
    const GIT_EVENT = 'waypoint-git://changed';
    function cmd(name, args) {
        return core.invoke(`${PREFIX}${name}`, args);
    }
    /** What the plugin can do here; unavailable, with the reason, while the Settings switch has Git off. */
    function getStatus() {
        return cmd('get_status');
    }
    /**
     * Starts hearing about the repository that holds `location` and replies with where it stands, or
     * `null` when the folder is not in a working tree, is not a local folder, or Git is off. Changes
     * follow as `GIT_EVENT`; read this reply first and apply events whose `id` is the watch's and whose
     * revision is higher.
     */
    function gitWatch(location) {
        return cmd('git_watch', { location });
    }
    /** Stops one watch. */
    function gitUnwatch(id) {
        return cmd('git_unwatch', { id });
    }
    /** How many paths changed inside each of `locations` that is a folder in a working tree; the others have no entry. */
    function gitBadges(locations) {
        return cmd('git_badges', { locations });
    }
    /**
     * The newest commits that changed a file or folder and how much of it changed since `HEAD`, or `null`
     * outside a working tree. Read it once a selection settles: it walks history.
     */
    function gitPathInfo(location, limit) {
        return cmd('git_path_info', { location, limit });
    }
    /** Hears every change of the repositories this window watches. */
    function onGitChanged(listener) {
        return event.listen(GIT_EVENT, (e) => listener(e.payload));
    }

    exports.GIT_EVENT = GIT_EVENT;
    exports.getStatus = getStatus;
    exports.gitBadges = gitBadges;
    exports.gitPathInfo = gitPathInfo;
    exports.gitUnwatch = gitUnwatch;
    exports.gitWatch = gitWatch;
    exports.onGitChanged = onGitChanged;

    return exports;

})({}, __TAURI__.core, __TAURI__.event);
Object.defineProperty(window.__TAURI__, 'waypointGit', { value: __TAURI_PLUGIN_WAYPOINT_GIT__ }) }
