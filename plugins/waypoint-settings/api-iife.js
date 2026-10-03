if ('__TAURI__' in window) {
var __TAURI_PLUGIN_WAYPOINT_SETTINGS__ = (function (exports, core, event) {
    'use strict';

    // Exposes typed guest-side wrappers for the waypoint-settings plugin
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PREFIX = 'plugin:waypoint-settings|';
    /** Sent to every window after every change; the payload is a `SettingsSnapshot`. */
    const SETTINGS_EVENT = 'waypoint-settings://changed';
    function cmd(name, args) {
        return core.invoke(`${PREFIX}${name}`, args);
    }
    /** Reports whether the settings plugin works. */
    function getStatus() {
        return cmd('get_status');
    }
    /** The settings in force and their revision. */
    function getSettings() {
        return cmd('get_settings');
    }
    /**
     * Saves new settings and returns what is in force. Rejects with a `SettingsCommandError` and
     * changes nothing for a value out of range or a save that fails.
     */
    function setSettings(settings) {
        return cmd('set_settings', { settings });
    }
    /**
     * Changes only the `ui` settings named in `change`, on top of what is in force now (Rust merges
     * them), so a window with an older copy of the document cannot turn other settings back. The
     * main windows may call this; they may not call `setSettings`.
     */
    function setUiSettings(change) {
        return cmd('set_ui_settings', { change });
    }
    /** Hears every change. Read `getSettings` first and apply snapshots with a higher revision. */
    function onSettingsChanged(listener) {
        return event.listen(SETTINGS_EVENT, (e) => listener(e.payload));
    }

    exports.SETTINGS_EVENT = SETTINGS_EVENT;
    exports.getSettings = getSettings;
    exports.getStatus = getStatus;
    exports.onSettingsChanged = onSettingsChanged;
    exports.setSettings = setSettings;
    exports.setUiSettings = setUiSettings;

    return exports;

})({}, __TAURI__.core, __TAURI__.event);
Object.defineProperty(window.__TAURI__, 'waypointSettings', { value: __TAURI_PLUGIN_WAYPOINT_SETTINGS__ }) }
