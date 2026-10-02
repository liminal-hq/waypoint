if ('__TAURI__' in window) {
var __TAURI_PLUGIN_MIME_APPS__ = (function (exports, core) {
    'use strict';

    // Exposes typed guest-side wrappers for the mime-apps plugin
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PREFIX = 'plugin:mime-apps|';
    function cmd(name, args) {
        return core.invoke(`${PREFIX}${name}`, args);
    }
    /** Reports which features work on this system, each with a typed reason when it does not, and which implementation is behind them. */
    function getStatus() {
        return cmd('get_status');
    }
    /** True if the feature is available. Decide behaviour from the features, never from the platform. */
    function hasFeature(status, feature) {
        return status.features.some((entry) => entry.name === feature && entry.available);
    }
    /** Why a feature is unavailable, as a code to branch on (`flatpak-sandbox`, `no-system-chooser`, `managed-by-system`, …), or `undefined` when it works. */
    function featureReason(status, feature) {
        return status.features.find((entry) => entry.name === feature)?.reason ?? undefined;
    }
    /** A sentence that explains why a feature is unavailable, or `undefined` when it works. */
    function featureMessage(status, feature) {
        return status.features.find((entry) => entry.name === feature)?.message ?? undefined;
    }
    /**
     * The type of a path or URI (`file:///…`, `smb://…`, a plain absolute path): its type, a phrase for it and an icon name.
     * A name ending in a slash, or a local directory, is `inode/directory`. The start of a local file is read only when
     * `sniff` is true.
     */
    function typeInfo(uri, sniff = false) {
        return cmd('type_info', { uri, sniff });
    }
    /**
     * The applications for the locations' type: the default, the ones registered for it and the others that can open it.
     * For locations of different types only the applications that open all of them are listed and `mixed` is true.
     */
    function handlers(uris) {
        return cmd('handlers', { uris });
    }
    /** Opens the locations in the application with this id (an `App.id`). Rejects with a `MimeAppsError`. */
    async function openWith(uris, appId) {
        await cmd('open_with', { uris, appId });
    }
    /** Opens each location in its default application. Rejects with `{ kind: 'noHandler', mime }` when a type has none; nothing is opened then. */
    async function openDefault(uris) {
        await cmd('open_default', { uris });
    }
    /**
     * Asks the system to let the person choose an application (on Windows the Open With dialog, as a child of the window
     * labelled `parentLabel`; in a Flatpak sandbox the portal's chooser). Rejects with `{ kind: 'cancelled' }` when it is
     * dismissed and `{ kind: 'unsupported' }` when the system has no chooser: draw a list from `handlers` then.
     */
    async function choose(uris, parentLabel) {
        await cmd('choose', { uris, parentLabel });
    }
    /** Makes an application the default for a type (`image/png`). Rejects with `{ kind: 'unsupported' }` where the system does not allow it. */
    async function setDefault(mime, appId) {
        await cmd('set_default', { mime, appId });
    }
    /** Opens the system's own page for default applications (Windows Settings). Rejects with `{ kind: 'unsupported' }` elsewhere. */
    async function openDefaultAppsSettings() {
        await cmd('open_default_apps_settings');
    }
    /**
     * The address of an application's icon, for an `<img src>`, served by the `appicon://` scheme by app id only. The
     * picture is a PNG at about `size` pixels (16 to 256); it is a 404 when the id is unknown or has no icon.
     */
    function appIconUrl(appId, size = 32) {
        const base = isWindowsWebview() ? 'http://appicon.localhost' : 'appicon://localhost';
        return `${base}/${encodeURIComponent(appId)}?size=${size}`;
    }
    function isWindowsWebview() {
        return typeof navigator !== 'undefined' && /Windows/i.test(navigator.userAgent);
    }
    /** True if a rejected value is a `MimeAppsError`. */
    function isMimeAppsError(value) {
        return (typeof value === 'object' && value !== null && typeof value.kind === 'string');
    }

    exports.appIconUrl = appIconUrl;
    exports.choose = choose;
    exports.featureMessage = featureMessage;
    exports.featureReason = featureReason;
    exports.getStatus = getStatus;
    exports.handlers = handlers;
    exports.hasFeature = hasFeature;
    exports.isMimeAppsError = isMimeAppsError;
    exports.openDefault = openDefault;
    exports.openDefaultAppsSettings = openDefaultAppsSettings;
    exports.openWith = openWith;
    exports.setDefault = setDefault;
    exports.typeInfo = typeInfo;

    return exports;

})({}, __TAURI__.core);
Object.defineProperty(window.__TAURI__, 'mimeApps', { value: __TAURI_PLUGIN_MIME_APPS__ }) }
