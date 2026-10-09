if ('__TAURI__' in window) {
var __TAURI_PLUGIN_ELEVATE__ = (function (exports, core) {
    'use strict';

    // Exposes typed guest-side wrappers for the elevate plugin: the status only, never a launch
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PREFIX = 'plugin:elevate|';
    function cmd(name, args) {
        return core.invoke(`${PREFIX}${name}`, args);
    }
    /**
     * Reports whether a helper can be started with administrator rights on this system, with a reason for
     * the person when it cannot. Starting one is not exposed to the page on purpose: only Rust code can.
     */
    function getStatus() {
        return cmd('get_status');
    }
    /** True if the feature is available. Decide behaviour from the features, never from the platform. */
    function hasFeature(status, feature) {
        return status.features.some((entry) => entry.name === feature && entry.available);
    }
    /** Why a feature is unavailable, or `undefined` when it works. */
    function featureReason(status, feature) {
        return status.features.find((entry) => entry.name === feature)?.reason ?? undefined;
    }

    exports.featureReason = featureReason;
    exports.getStatus = getStatus;
    exports.hasFeature = hasFeature;

    return exports;

})({}, __TAURI__.core);
Object.defineProperty(window.__TAURI__, 'elevate', { value: __TAURI_PLUGIN_ELEVATE__ }) }
