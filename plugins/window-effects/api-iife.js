if ('__TAURI__' in window) {
var __TAURI_PLUGIN_WINDOW_EFFECTS__ = (function (exports, core) {
    'use strict';

    // Exposes typed guest-side wrappers for the window-effects plugin
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PREFIX = 'plugin:window-effects|';
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
    /** Why a feature is unavailable, as a code to branch on (`compositor-has-no-blur`, `needs-windows-11`, `x11-no-compositor`, …), or `undefined` when it works. */
    function featureReason(status, feature) {
        return status.features.find((entry) => entry.name === feature)?.reason ?? undefined;
    }
    /** A sentence that explains why a feature is unavailable, or `undefined` when it works. */
    function featureMessage(status, feature) {
        return status.features.find((entry) => entry.name === feature)?.message ?? undefined;
    }
    /**
     * Puts an effect behind the window with that label, replacing the one it had. How see-through the
     * window is stays the page's own alpha: the effect shows only where the page is transparent. Apply
     * again when the theme changes (`dark`) or, with a `region`, when the window is resized. Rejects
     * with a `WindowEffectsError`: `{ kind: 'unsupported', reason, message }` when this system cannot do it.
     */
    async function apply(label, effects) {
        await cmd('apply', { label, effects });
    }
    /** Takes the window's effect away. */
    async function clear(label) {
        await cmd('clear', { label });
    }
    /** Tells the compositor how much of the window, in logical pixels, is shadow or invisible border, so a tiled window sits flush. All zeros puts it back. */
    async function setShadowInset(label, insets) {
        await cmd('set_shadow_inset', { label, insets });
    }
    /** True if a rejected value is a `WindowEffectsError`. */
    function isWindowEffectsError(value) {
        return (typeof value === 'object' &&
            value !== null &&
            typeof value.kind === 'string');
    }

    exports.apply = apply;
    exports.clear = clear;
    exports.featureMessage = featureMessage;
    exports.featureReason = featureReason;
    exports.getStatus = getStatus;
    exports.hasFeature = hasFeature;
    exports.isWindowEffectsError = isWindowEffectsError;
    exports.setShadowInset = setShadowInset;

    return exports;

})({}, __TAURI__.core);
Object.defineProperty(window.__TAURI__, 'windowEffects', { value: __TAURI_PLUGIN_WINDOW_EFFECTS__ }) }
