if ('__TAURI__' in window) {
var __TAURI_PLUGIN_WINDOW_MANAGER__ = (function (exports, core, webviewWindow) {
    'use strict';

    // Exposes typed guest-side wrappers for the window manager plugin
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PREFIX = 'plugin:window-manager|';
    const ALWAYS_ON_TOP_CHANGED_EVENT = 'window-manager://always-on-top-changed';
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
    /**
     * Whether the window manager is keeping the calling window above others, read from the window manager rather than echoed from the last request.
     * Resolves to null where it cannot be observed: Wayland has no such state, and unsupported targets have no window manager integration.
     */
    function getAlwaysOnTop() {
        return cmd('get_always_on_top');
    }
    /**
     * Listens for the window manager changing whether the calling window is kept above others, including through its own window menu.
     * Only X11 reports these changes. Subscribe first and then call `getAlwaysOnTop()`, so a change between the two cannot be missed.
     */
    function onAlwaysOnTopChanged(handler) {
        return webviewWindow.getCurrentWebviewWindow().listen(ALWAYS_ON_TOP_CHANGED_EVENT, (event) => handler(event.payload));
    }

    exports.getAlwaysOnTop = getAlwaysOnTop;
    exports.getCapabilities = getCapabilities;
    exports.getStatus = getStatus;
    exports.onAlwaysOnTopChanged = onAlwaysOnTopChanged;
    exports.showSystemWindowMenu = showSystemWindowMenu;

    return exports;

})({}, __TAURI__.core, __TAURI__.webviewWindow);
Object.defineProperty(window.__TAURI__, 'windowManager', { value: __TAURI_PLUGIN_WINDOW_MANAGER__ }) }
