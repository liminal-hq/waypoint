if ('__TAURI__' in window) {
var __TAURI_PLUGIN_WAYPOINT_SESSION__ = (function (exports, core, event) {
    'use strict';

    // Exposes typed guest-side wrappers for the waypoint-session plugin
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PREFIX = 'plugin:waypoint-session|';
    const EVENT = 'waypoint-session://event';
    function cmd(name, args) {
        return core.invoke(`${PREFIX}${name}`, args);
    }
    /** Reports whether the session plugin works and which features it offers. */
    function getStatus() {
        return cmd('get_status');
    }
    /** The calling window's whole session at its current revision. */
    function getSnapshot() {
        return cmd('get_snapshot');
    }
    /** Opens a tab after `after` (or at the end) and returns its id. */
    function openTab(location, after, activate = true) {
        return cmd('open_tab', { location, after: after ?? null, activate });
    }
    function closeTab(tab) {
        return cmd('close_tab', { tab });
    }
    function activateTab(tab) {
        return cmd('activate_tab', { tab });
    }
    function moveTab(tab, index) {
        return cmd('move_tab', { tab, index });
    }
    /** Goes somewhere new in a tab: the current location joins its back history. */
    function navigate(tab, location) {
        return cmd('navigate', { tab, location });
    }
    function back(tab) {
        return cmd('back', { tab });
    }
    function forward(tab) {
        return cmd('forward', { tab });
    }
    /** Follows every change to the calling window's session. */
    function onTabsEvent(listener) {
        return event.listen(EVENT, (e) => listener(e.payload));
    }

    exports.activateTab = activateTab;
    exports.back = back;
    exports.closeTab = closeTab;
    exports.forward = forward;
    exports.getSnapshot = getSnapshot;
    exports.getStatus = getStatus;
    exports.moveTab = moveTab;
    exports.navigate = navigate;
    exports.onTabsEvent = onTabsEvent;
    exports.openTab = openTab;

    return exports;

})({}, __TAURI__.core, __TAURI__.event);
Object.defineProperty(window.__TAURI__, 'waypointSession', { value: __TAURI_PLUGIN_WAYPOINT_SESSION__ }) }
