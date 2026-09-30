if ('__TAURI__' in window) {
var __TAURI_PLUGIN_SYSTEM_APPEARANCE__ = (function (exports, core, event) {
    'use strict';

    // Exposes typed guest-side wrappers for the system appearance plugin
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PREFIX = 'plugin:system-appearance|';
    /** Event emitted to all windows when the titlebar preferences change. */
    const TITLEBAR_PREFERENCES_CHANGED_EVENT = 'system-appearance://titlebar-preferences-changed';
    function cmd(name, args) {
        return core.invoke(`${PREFIX}${name}`, args);
    }
    /** Reports whether the platform's preferences could be read and which sources worked. */
    function getStatus() {
        return cmd('get_status');
    }
    /**
     * Reads the current titlebar preferences, stamped with a revision; falls back to a default with
     * source `default`. Keep the highest revision seen and ignore anything older, because a change
     * event and a read can arrive in either order.
     */
    function getTitlebarPreferences() {
        return cmd('get_titlebar_preferences');
    }
    /** Subscribes to preference changes and resolves to a function that unsubscribes. */
    function onTitlebarPreferencesChanged(callback) {
        return event.listen(TITLEBAR_PREFERENCES_CHANGED_EVENT, (event) => callback(event.payload));
    }

    exports.TITLEBAR_PREFERENCES_CHANGED_EVENT = TITLEBAR_PREFERENCES_CHANGED_EVENT;
    exports.getStatus = getStatus;
    exports.getTitlebarPreferences = getTitlebarPreferences;
    exports.onTitlebarPreferencesChanged = onTitlebarPreferencesChanged;

    return exports;

})({}, __TAURI__.core, __TAURI__.event);
Object.defineProperty(window.__TAURI__, 'systemAppearance', { value: __TAURI_PLUGIN_SYSTEM_APPEARANCE__ }) }
