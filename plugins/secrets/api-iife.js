if ('__TAURI__' in window) {
var __TAURI_PLUGIN_SECRETS__ = (function (exports, core) {
    'use strict';

    // Exposes typed guest-side wrappers for the secrets plugin
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PREFIX = 'plugin:secrets|';
    function cmd(name, args) {
        return core.invoke(`${PREFIX}${name}`, args);
    }
    /** Reports which features work on this system, each with a typed reason when it does not, and which keyring is behind them. It never prompts to unlock the keyring. */
    function getStatus() {
        return cmd('get_status');
    }
    /** True if the feature is available. Decide behaviour from the features, never from the platform. */
    function hasFeature(status, feature) {
        return status.features.some((entry) => entry.name === feature && entry.available);
    }
    /** Why a feature is unavailable, as a code to branch on (`no-keyring`, `locked`, …), or `undefined` when it works. */
    function featureReason(status, feature) {
        return status.features.find((entry) => entry.name === feature)?.reason ?? undefined;
    }
    /** A sentence that explains why a feature is unavailable, or `undefined` when it works. */
    function featureMessage(status, feature) {
        return status.features.find((entry) => entry.name === feature)?.message ?? undefined;
    }
    /**
     * Stores a secret, replacing one with the same id. The value crosses the bridge once, is neither
     * logged nor kept by the plugin and is not returned. `label` is what the keyring's own manager shows.
     * Rejects with a `SecretsError`.
     */
    async function store(id, secret, label) {
        await cmd('store', { id, secret, label });
    }
    /**
     * Reads a secret back, or `null` when there is none. Needs the separate `secrets:allow-fetch`
     * permission, which the plugin's default set leaves out: prefer to use a secret in Rust.
     */
    function fetch(id) {
        return cmd('fetch', { id });
    }
    /** Whether a secret exists, without reading it. */
    function exists(id) {
        return cmd('exists', { id });
    }
    /** Deletes a secret; true when there was one. */
    function remove(id) {
        return cmd('delete', { id });
    }
    /** Deletes every kind of secret of one service and account and returns how many there were. */
    function removeAccount(service, account) {
        return cmd('delete_account', { service, account });
    }
    /** True if a rejected value is a `SecretsError`. */
    function isSecretsError(value) {
        return (typeof value === 'object' && value !== null && typeof value.kind === 'string');
    }

    exports.exists = exists;
    exports.featureMessage = featureMessage;
    exports.featureReason = featureReason;
    exports.fetch = fetch;
    exports.getStatus = getStatus;
    exports.hasFeature = hasFeature;
    exports.isSecretsError = isSecretsError;
    exports.remove = remove;
    exports.removeAccount = removeAccount;
    exports.store = store;

    return exports;

})({}, __TAURI__.core);
Object.defineProperty(window.__TAURI__, 'secrets', { value: __TAURI_PLUGIN_SECRETS__ }) }
