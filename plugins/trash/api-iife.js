if ('__TAURI__' in window) {
var __TAURI_PLUGIN_TRASH__ = (function (exports, core) {
    'use strict';

    // Exposes typed guest-side wrappers for the trash plugin
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PREFIX = 'plugin:trash|';
    function cmd(name, args) {
        return core.invoke(`${PREFIX}${name}`, args);
    }
    /** Reports which features work on this system, each with a reason when it does not, and which implementation is behind them. */
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
    /**
     * Moves each absolute path to the trash, as one batch. The result has one outcome per path, in
     * order: a path that fails does not stop the others, and every failure is reported.
     */
    function trash(paths) {
        return cmd('trash', { paths });
    }
    /** Everything in the trash, oldest first. Rejects with a `TrashError` (`unsupported` where `list` is unavailable). */
    function list() {
        return cmd('list');
    }
    /**
     * Puts an item back where it was trashed from (the default) or at a full path of the caller's choice.
     * Never overwrites: rejects with `originExists` when the place is taken and `originMissingParent`
     * when its folder is gone, so the caller can prompt and try again with `{ kind: 'path', path }`.
     */
    function restore(receipt, target = { kind: 'original' }) {
        return cmd('restore', { receipt, target });
    }
    /** Removes one item from the trash for good. */
    async function deleteItem(receipt) {
        await cmd('delete', { receipt });
    }
    /**
     * Empties the trash, or with `olderThanDays` only the items trashed that many days ago or earlier.
     * Items that cannot be removed are listed in `failed` and stay in the trash.
     */
    function empty(olderThanDays) {
        return cmd('empty', { olderThanDays: olderThanDays ?? null });
    }
    /** True if a rejected value is a `TrashError`. */
    function isTrashError(value) {
        return (typeof value === 'object' && value !== null && typeof value.kind === 'string');
    }

    exports.deleteItem = deleteItem;
    exports.empty = empty;
    exports.featureReason = featureReason;
    exports.getStatus = getStatus;
    exports.hasFeature = hasFeature;
    exports.isTrashError = isTrashError;
    exports.list = list;
    exports.restore = restore;
    exports.trash = trash;

    return exports;

})({}, __TAURI__.core);
Object.defineProperty(window.__TAURI__, 'trash', { value: __TAURI_PLUGIN_TRASH__ }) }
