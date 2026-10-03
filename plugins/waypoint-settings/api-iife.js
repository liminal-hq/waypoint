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
    /**
     * Asks where to save with the system's dialog and writes the settings there: a `.json` when one
     * configuration file is exported and a `.zip` when there are several. `null` when the dialog was
     * closed. `utcOffsetMinutes` (as `-new Date().getTimezoneOffset()`) puts the person's own date in
     * the suggested name. Only the Settings window may call this.
     */
    function exportSettings(utcOffsetMinutes) {
        return cmd('export_settings', { utcOffsetMinutes });
    }
    /**
     * Asks which file to read with the system's dialog and plans importing it, changing nothing.
     * `null` when the dialog was closed. Only the Settings window may call this.
     */
    function planSettingsImport() {
        return cmd('plan_settings_import');
    }
    /**
     * Applies the plan just made (all or nothing, as one change every window hears) and returns what
     * is now in force. Rust reads the file again; the page sends only the plan's number. Only the
     * Settings window may call this.
     */
    function applySettingsImport(planId) {
        return cmd('apply_settings_import', { planId });
    }
    /** Hears every change. Read `getSettings` first and apply snapshots with a higher revision. */
    function onSettingsChanged(listener) {
        return event.listen(SETTINGS_EVENT, (e) => listener(e.payload));
    }

    exports.SETTINGS_EVENT = SETTINGS_EVENT;
    exports.applySettingsImport = applySettingsImport;
    exports.exportSettings = exportSettings;
    exports.getSettings = getSettings;
    exports.getStatus = getStatus;
    exports.onSettingsChanged = onSettingsChanged;
    exports.planSettingsImport = planSettingsImport;
    exports.setSettings = setSettings;
    exports.setUiSettings = setUiSettings;

    return exports;

})({}, __TAURI__.core, __TAURI__.event);
Object.defineProperty(window.__TAURI__, 'waypointSettings', { value: __TAURI_PLUGIN_WAYPOINT_SETTINGS__ }) }
