if ('__TAURI__' in window) {
var __TAURI_PLUGIN_WINDOW_TEAROFF__ = (function (exports, core, webviewWindow) {
    'use strict';

    // Exposes typed guest-side wrappers for the window tear-off plugin
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PREFIX = 'plugin:window-tearoff|';
    /** Sent to the ghost window with the drag's payload. */
    const PAYLOAD_EVENT = 'window-tearoff://payload';
    /** Sent to the window that began a drag when it ran too long and was ended. */
    const TIMEOUT_EVENT = 'window-tearoff://timeout';
    /** Sent to the window that began a drag when the cursor value froze (`true`) or moved again (`false`). */
    const CURSOR_STALE_EVENT = 'window-tearoff://cursor-stale';
    function cmd(name, args) {
        return core.invoke(`${PREFIX}${name}`, args);
    }
    /**
     * Reports which tear-off features work on this system, and why the others do not.
     * The first call probes the windowing system and can take up to a second, so call it once at startup.
     */
    function getStatus() {
        return cmd('get_status');
    }
    /** True if `feature` is in `status.features`. */
    function hasFeature(status, feature) {
        return status.features.includes(feature);
    }
    /**
     * Starts a drag: shows the ghost under the cursor with `payload` and follows the cursor until `end`.
     * `grabOffset` is where inside the dragged thing the user grabbed it, in logical pixels from its top-left; `size` is the ghost's logical size.
     * Resolves to `{ state: 'noGhost' }` where no ghost can be shown (render a preview in the page instead), and `{ state: 'alreadyActive' }` if a drag is running.
     */
    function begin(payload, grabOffset, size) {
        return cmd('begin', { payload, grabOffset, size });
    }
    /** Replaces the ghost's payload while a drag is in progress. */
    function update(payload) {
        return cmd('update', { payload });
    }
    /**
     * Ends the drag and hides the ghost.
     * Resolves to where the cursor was (physical pixels) and the registered region it was over; a cancelled drag never has a hit.
     */
    function end(outcome) {
        return cmd('end', { outcome });
    }
    /** Registers the calling window's drop regions, in logical pixels from the top-left of its content, replacing its earlier ones. */
    function setDropRegions(regions) {
        return cmd('set_drop_regions', { regions });
    }
    /** The native cursor in physical pixels, or null where the system does not report a usable one. */
    function getCursor() {
        return cmd('get_cursor');
    }
    /**
     * The registered region under the cursor right now, without ending the drag, so a caller can say what a release would do.
     * Null where the system reports no usable cursor, cannot hit-test, or the cursor is over no region.
     */
    function hitTest() {
        return cmd('hit_test');
    }
    /** The payload of the drag in progress, so a ghost page that loaded after `begin` can still draw it. */
    function getPayload() {
        return cmd('get_payload');
    }
    /** Listens, in the ghost window, for the drag's payload as it is sent and updated; `null` means the drag ended and the card should clear. */
    function onPayload(handler) {
        return webviewWindow.getCurrentWebviewWindow().listen(PAYLOAD_EVENT, (event) => handler(event.payload));
    }
    /** Listens for a drag the plugin ended because it ran too long. */
    function onTimeout(handler) {
        return webviewWindow.getCurrentWebviewWindow().listen(TIMEOUT_EVENT, () => handler());
    }
    /** Listens for the cursor value freezing while a button is held (`true`) and moving again (`false`). */
    function onCursorStale(handler) {
        return webviewWindow.getCurrentWebviewWindow().listen(CURSOR_STALE_EVENT, (event) => handler(event.payload));
    }

    exports.CURSOR_STALE_EVENT = CURSOR_STALE_EVENT;
    exports.PAYLOAD_EVENT = PAYLOAD_EVENT;
    exports.TIMEOUT_EVENT = TIMEOUT_EVENT;
    exports.begin = begin;
    exports.end = end;
    exports.getCursor = getCursor;
    exports.getPayload = getPayload;
    exports.getStatus = getStatus;
    exports.hasFeature = hasFeature;
    exports.hitTest = hitTest;
    exports.onCursorStale = onCursorStale;
    exports.onPayload = onPayload;
    exports.onTimeout = onTimeout;
    exports.setDropRegions = setDropRegions;
    exports.update = update;

    return exports;

})({}, __TAURI__.core, __TAURI__.webviewWindow);
Object.defineProperty(window.__TAURI__, 'windowTearoff', { value: __TAURI_PLUGIN_WINDOW_TEAROFF__ }) }
