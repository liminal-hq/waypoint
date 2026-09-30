if ('__TAURI__' in window) {
var __TAURI_PLUGIN_WINDOW_MANAGER__ = (function (exports, core) {
    'use strict';

    // Exposes typed guest-side wrappers for the window manager plugin
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PREFIX = 'plugin:window-manager|';
    function cmd(name, args) {
        return core.invoke(`${PREFIX}${name}`, args);
    }
    /** Reports whether any window manager feature works on this system, and which. */
    function getStatus() {
        return cmd('get_status');
    }
    /** Reports the windowing system and which window manager features the app can use. */
    function getCapabilities() {
        return cmd('get_capabilities');
    }
    /**
     * Asks the compositor to show its own window menu for the calling window at `position` (CSS pixels from the window's top-left).
     * Resolves to true if the compositor accepted the request, and false if it is unsupported or was refused. Wayland only honours it right after a real mouse press, so call it from a pointer event handler.
     */
    function showSystemWindowMenu(position) {
        return cmd('show_system_window_menu', { position });
    }

    exports.getCapabilities = getCapabilities;
    exports.getStatus = getStatus;
    exports.showSystemWindowMenu = showSystemWindowMenu;

    return exports;

})({}, __TAURI__.core);
Object.defineProperty(window.__TAURI__, 'windowManager', { value: __TAURI_PLUGIN_WINDOW_MANAGER__ }) }
