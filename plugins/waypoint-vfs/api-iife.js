if ('__TAURI__' in window) {
var __TAURI_PLUGIN_WAYPOINT_VFS__ = (function (exports, core) {
    'use strict';

    // Exposes typed guest-side wrappers for the waypoint-vfs plugin
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PREFIX = 'plugin:waypoint-vfs|';
    /** Reports whether the file system plugin can do anything yet, and which features work. */
    function getStatus() {
        return core.invoke(`${PREFIX}get_status`);
    }

    exports.getStatus = getStatus;

    return exports;

})({}, __TAURI__.core);
Object.defineProperty(window.__TAURI__, 'waypointVfs', { value: __TAURI_PLUGIN_WAYPOINT_VFS__ }) }
