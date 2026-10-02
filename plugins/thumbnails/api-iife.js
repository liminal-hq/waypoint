if ('__TAURI__' in window) {
var __TAURI_PLUGIN_THUMBNAILS__ = (function (exports, core) {
    'use strict';

    // Exposes typed guest-side wrappers for the thumbnails plugin
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PREFIX = 'plugin:thumbnails|';
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
    /** Why a feature is unavailable, or `undefined` when it works. */
    function featureReason(status, feature) {
        return status.features.find((entry) => entry.name === feature)?.reason ?? undefined;
    }
    /**
     * Asks for thumbnails. Each result arrives through `onEvent` as it is made, the newest request
     * first; a key requested twice is one job. A `ready` event's `url` names a cache entry and is
     * loaded like any image address; it never contains a path. Returns the ticket to cancel or
     * reprioritise with.
     */
    function request(items, onEvent) {
        const channel = new core.Channel();
        channel.onmessage = onEvent;
        return cmd('request', { items, onReady: channel });
    }
    /** Withdraws a request: nothing more is sent to it, and its unstarted work is dropped. True if the ticket was still known. */
    function cancel(ticket) {
        return cmd('cancel', { ticket });
    }
    /** Moves the request's pending items for `keys` to the front of the queue, in the order given. Call it when the visible rows change. */
    async function prioritise(ticket, keys) {
        await cmd('prioritise', { ticket, keys });
    }

    exports.cancel = cancel;
    exports.featureReason = featureReason;
    exports.getStatus = getStatus;
    exports.hasFeature = hasFeature;
    exports.prioritise = prioritise;
    exports.request = request;

    return exports;

})({}, __TAURI__.core);
Object.defineProperty(window.__TAURI__, 'thumbnails', { value: __TAURI_PLUGIN_THUMBNAILS__ }) }
