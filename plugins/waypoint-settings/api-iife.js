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
    /** Sent to every window after every change to the remembered folder views; the payload is a `FolderViewsChanged`. */
    const FOLDER_VIEWS_EVENT = 'waypoint-settings://folder-views';
    /** Every remembered folder view and the revision they are at. */
    function getFolderViews() {
        return cmd('get_folder_views');
    }
    /**
     * Remembers the view, sort or grouping chosen for the folder at `key` (its location's `uri`), on
     * top of what it already remembers: a field left `null` is kept as it is. Resolves to the
     * revision in force. Rejects with a `SettingsCommandError` (`invalid` for a location that cannot
     * be one, `storage` for a save that failed) and changes nothing.
     */
    function rememberFolderView(key, patch) {
        return cmd('remember_folder_view', { key, patch });
    }
    /** Makes the folder at `key` forget its own view, so it shows the window's again. Resolves to the revision in force. */
    function resetFolderView(key) {
        return cmd('reset_folder_view', { key });
    }
    /**
     * Hears every change to the remembered views. Read `getFolderViews` first; an event whose
     * revision is not the next one means a change was missed, and the snapshot is read again.
     */
    function onFolderViewsChanged(listener) {
        return event.listen(FOLDER_VIEWS_EVENT, (e) => listener(e.payload));
    }

    exports.FOLDER_VIEWS_EVENT = FOLDER_VIEWS_EVENT;
    exports.SETTINGS_EVENT = SETTINGS_EVENT;
    exports.applySettingsImport = applySettingsImport;
    exports.exportSettings = exportSettings;
    exports.getFolderViews = getFolderViews;
    exports.getSettings = getSettings;
    exports.getStatus = getStatus;
    exports.onFolderViewsChanged = onFolderViewsChanged;
    exports.onSettingsChanged = onSettingsChanged;
    exports.planSettingsImport = planSettingsImport;
    exports.rememberFolderView = rememberFolderView;
    exports.resetFolderView = resetFolderView;
    exports.setSettings = setSettings;
    exports.setUiSettings = setUiSettings;

    return exports;

})({}, __TAURI__.core, __TAURI__.event);
Object.defineProperty(window.__TAURI__, 'waypointSettings', { value: __TAURI_PLUGIN_WAYPOINT_SETTINGS__ }) }
