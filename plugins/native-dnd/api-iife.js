if ('__TAURI__' in window) {
var __TAURI_PLUGIN_NATIVE_DND__ = (function (exports, core, webviewWindow) {
    'use strict';

    // Exposes typed guest-side wrappers for the native-dnd plugin
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PREFIX = 'plugin:native-dnd|';
    /** Sent to a window when files first move over it. */
    const ENTER_EVENT = 'native-dnd://enter';
    /** Sent to a window as files move over it. */
    const OVER_EVENT = 'native-dnd://over';
    /** Sent to a window when files are dropped on it. */
    const DROP_EVENT = 'native-dnd://drop';
    /** Sent to a window when files leave it without being dropped. */
    const LEAVE_EVENT = 'native-dnd://leave';
    /** Sent to the window that started an outbound drag when it ends however it ends. */
    const DRAG_ENDED_EVENT = 'native-dnd://drag-ended';
    /** Sent to every window when the file clipboard may have changed. */
    const CLIPBOARD_CHANGED_EVENT = 'native-dnd://clipboard-changed';
    function cmd(name, args) {
        return core.invoke(`${PREFIX}${name}`, args);
    }
    /**
     * Reports which native drag and drop features work on this system, and why the others do not.
     * Where nothing works every feature says why and the commands reject with `unsupported`.
     */
    function getStatus() {
        return cmd('get_status');
    }
    /** True if `feature` works according to `status`. */
    function hasFeature(status, feature) {
        return status.features[feature].available;
    }
    /**
     * Starts an outbound drag of `request.uris` (`file://` URIs) from the calling window, which must be in the middle of a press of the primary mouse button.
     * Rejects with `buttonNotPressed` if it is not, `alreadyActive` while another drag runs, `invalid` for a malformed request and `unsupported` where outbound drags do not work.
     * On Linux it resolves as soon as the drag has started (`ended` is null) and `onDragEnded` reports the end; on Windows it resolves when the drag has finished, with `ended` set (`onDragEnded` still fires).
     * The page gets no pointer events from the moment the drag starts, so reset its own pointer state in `onDragEnded`.
     */
    function startDrag(request) {
        return cmd('start_drag', { request });
    }
    /**
     * Puts `files` on the system clipboard, to copy or (with `cut`) to move.
     * On Wayland the compositor accepts it only shortly after a key press or click in the app, so call it from the handler of that action.
     */
    function setFiles(files) {
        return cmd('set_files', { files });
    }
    /** The files on the system clipboard, or null when it holds none. */
    function getFiles() {
        return cmd('get_files');
    }
    /** True if `error` is the value a plugin command rejects with, optionally of the given `kind`. */
    function isNativeDndError(error, kind) {
        if (typeof error !== 'object' || error === null)
            return false;
        const candidate = error;
        return (typeof candidate.kind === 'string' &&
            typeof candidate.message === 'string' &&
            (kind === undefined || candidate.kind === kind));
    }
    /** Listens, in the calling window, for files first moving over it. Only windows whose drag-drop handler is on get these. */
    function onEnter(handler) {
        return webviewWindow.getCurrentWebviewWindow().listen(ENTER_EVENT, (event) => handler(event.payload));
    }
    /** Listens, in the calling window, for files moving over it (every pointer move, so keep the handler cheap). */
    function onOver(handler) {
        return webviewWindow.getCurrentWebviewWindow().listen(OVER_EVENT, (event) => handler(event.payload));
    }
    /** Listens, in the calling window, for a drop. `selfDrop` marks the end of a drag this app started. */
    function onDrop(handler) {
        return webviewWindow.getCurrentWebviewWindow().listen(DROP_EVENT, (event) => handler(event.payload));
    }
    /** Listens, in the calling window, for files leaving it without a drop. */
    function onLeave(handler) {
        return webviewWindow.getCurrentWebviewWindow().listen(LEAVE_EVENT, (event) => handler(event.payload));
    }
    /** Listens, in the window that started an outbound drag, for its end. */
    function onDragEnded(handler) {
        return webviewWindow.getCurrentWebviewWindow().listen(DRAG_ENDED_EVENT, (event) => handler(event.payload));
    }
    /** Listens for the clipboard's owner changing, which may mean `getFiles` now answers differently. */
    function onClipboardChanged(handler) {
        return webviewWindow.getCurrentWebviewWindow().listen(CLIPBOARD_CHANGED_EVENT, () => handler());
    }

    exports.CLIPBOARD_CHANGED_EVENT = CLIPBOARD_CHANGED_EVENT;
    exports.DRAG_ENDED_EVENT = DRAG_ENDED_EVENT;
    exports.DROP_EVENT = DROP_EVENT;
    exports.ENTER_EVENT = ENTER_EVENT;
    exports.LEAVE_EVENT = LEAVE_EVENT;
    exports.OVER_EVENT = OVER_EVENT;
    exports.getFiles = getFiles;
    exports.getStatus = getStatus;
    exports.hasFeature = hasFeature;
    exports.isNativeDndError = isNativeDndError;
    exports.onClipboardChanged = onClipboardChanged;
    exports.onDragEnded = onDragEnded;
    exports.onDrop = onDrop;
    exports.onEnter = onEnter;
    exports.onLeave = onLeave;
    exports.onOver = onOver;
    exports.setFiles = setFiles;
    exports.startDrag = startDrag;

    return exports;

})({}, __TAURI__.core, __TAURI__.webviewWindow);
Object.defineProperty(window.__TAURI__, 'nativeDnd', { value: __TAURI_PLUGIN_NATIVE_DND__ }) }
