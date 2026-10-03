if ('__TAURI__' in window) {
var __TAURI_PLUGIN_VOLUMES__ = (function (exports, core, event) {
    'use strict';

    // Exposes typed guest-side wrappers for the volumes plugin
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PREFIX = 'plugin:volumes|';
    /** The event name of `onChanged`. */
    const CHANGED_EVENT = 'volumes://changed';
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
    /** Why a feature is unavailable, as a code to branch on (`no-system-bus`, `udisks2-missing`, `flatpak-sandbox`, …), or `undefined` when it works. */
    function featureReason(status, feature) {
        return status.features.find((entry) => entry.name === feature)?.reason ?? undefined;
    }
    /** A sentence that explains why a feature is unavailable, or `undefined` when it works. */
    function featureMessage(status, feature) {
        return status.features.find((entry) => entry.name === feature)?.message ?? undefined;
    }
    /**
     * Every volume now. Mounted local volumes are measured within a timeout, so `free` is `null` for one
     * that did not answer. Network volumes are not measured unless `measure` is true; use `refreshSpace`
     * to measure one.
     */
    function list(measure = false) {
        return cmd('list', { measure });
    }
    /** Measures one volume, network or not, within the timeout, and returns it with its `total` and `free` when they were found. */
    function refreshSpace(id) {
        return cmd('refresh_space', { id });
    }
    /** Mounts a volume and returns its mount point. Rejects with a `VolumesError`. */
    function mount(id) {
        return cmd('mount', { id });
    }
    /** Unmounts a volume. Rejects with `{ kind: 'busy', by }` while something holds it. */
    async function unmount(id) {
        await cmd('unmount', { id });
    }
    /** Unmounts everything on the volume's drive and ejects it, powering the drive off when it allows. */
    async function eject(id) {
        await cmd('eject', { id });
    }
    /**
     * Unlocks an encrypted volume and returns the id of the volume that appears (mount it next). The
     * passphrase is sent once and neither logged nor kept; a wrong one rejects with `wrongPassphrase`.
     */
    function unlock(id, passphrase) {
        return cmd('unlock', { id, passphrase });
    }
    /**
     * Calls `handler` with the whole list each time it changes. Revisions only grow: ignore an event
     * whose `revision` is not above the last one you have seen.
     */
    function onChanged(handler) {
        return event.listen(CHANGED_EVENT, (event) => handler(event.payload));
    }
    /** True if a rejected value is a `VolumesError`. */
    function isVolumesError(value) {
        return (typeof value === 'object' && value !== null && typeof value.kind === 'string');
    }

    exports.CHANGED_EVENT = CHANGED_EVENT;
    exports.eject = eject;
    exports.featureMessage = featureMessage;
    exports.featureReason = featureReason;
    exports.getStatus = getStatus;
    exports.hasFeature = hasFeature;
    exports.isVolumesError = isVolumesError;
    exports.list = list;
    exports.mount = mount;
    exports.onChanged = onChanged;
    exports.refreshSpace = refreshSpace;
    exports.unlock = unlock;
    exports.unmount = unmount;

    return exports;

})({}, __TAURI__.core, __TAURI__.event);
Object.defineProperty(window.__TAURI__, 'volumes', { value: __TAURI_PLUGIN_VOLUMES__ }) }
