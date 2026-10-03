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
    /** Event emitted to all windows when the appearance preferences change. */
    const APPEARANCE_CHANGED_EVENT = 'system-appearance://appearance-changed';
    /** Event emitted to all windows when the colour palette changes. */
    const PALETTE_CHANGED_EVENT = 'system-appearance://palette-changed';
    function cmd(name, args) {
        return core.invoke(`${PREFIX}${name}`, args);
    }
    /**
     * Reports whether the platform's preferences could be read and which sources worked. `appearance`
     * lists every appearance feature, with a typed `reason` for each one that does not work here.
     */
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
    /**
     * Reads the current appearance preferences (colour scheme, accent, contrast, reduced motion and
     * transparency, text scale and icon theme), stamped with a revision, and the source that supplied
     * each one. A preference nothing could answer holds a neutral value and has no source; `getStatus`
     * says why. Keep the highest revision seen and ignore anything older.
     */
    function getAppearance() {
        return cmd('get_appearance');
    }
    /**
     * Reads the operating system's colour palette (window and view backgrounds and text, the raised
     * surface, the selection and its text, the border, the focus colour and the warning, error and
     * success colours), stamped with a revision. Each colour is a `PaletteEntry` with the `source`
     * that supplied it, or a typed `reason` and a `detail` when it is unavailable; `status` says
     * whether the palette can be used at all. Keep the highest revision seen and ignore anything older.
     */
    function getPalette() {
        return cmd('get_palette');
    }
    /** Subscribes to titlebar preference changes and resolves to a function that unsubscribes. */
    function onTitlebarPreferencesChanged(callback) {
        return event.listen(TITLEBAR_PREFERENCES_CHANGED_EVENT, (event) => callback(event.payload));
    }
    /**
     * Subscribes to appearance changes and resolves to a function that unsubscribes. Subscribe first
     * and read second, so a change made while starting up is not missed.
     */
    function onAppearanceChanged(callback) {
        return event.listen(APPEARANCE_CHANGED_EVENT, (event) => callback(event.payload));
    }
    /**
     * Subscribes to palette changes and resolves to a function that unsubscribes. Subscribe first and
     * read second, so a change made while starting up is not missed.
     */
    function onPaletteChanged(callback) {
        return event.listen(PALETTE_CHANGED_EVENT, (event) => callback(event.payload));
    }

    exports.APPEARANCE_CHANGED_EVENT = APPEARANCE_CHANGED_EVENT;
    exports.PALETTE_CHANGED_EVENT = PALETTE_CHANGED_EVENT;
    exports.TITLEBAR_PREFERENCES_CHANGED_EVENT = TITLEBAR_PREFERENCES_CHANGED_EVENT;
    exports.getAppearance = getAppearance;
    exports.getPalette = getPalette;
    exports.getStatus = getStatus;
    exports.getTitlebarPreferences = getTitlebarPreferences;
    exports.onAppearanceChanged = onAppearanceChanged;
    exports.onPaletteChanged = onPaletteChanged;
    exports.onTitlebarPreferencesChanged = onTitlebarPreferencesChanged;

    return exports;

})({}, __TAURI__.core, __TAURI__.event);
Object.defineProperty(window.__TAURI__, 'systemAppearance', { value: __TAURI_PLUGIN_SYSTEM_APPEARANCE__ }) }
