if ('__TAURI__' in window) {
var __TAURI_PLUGIN_OS_PREFS__ = (function (exports, core, event) {
    'use strict';

    // Exposes typed guest-side wrappers for the os-prefs plugin
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PREFIX = 'plugin:os-prefs|';
    /** Event emitted to all windows with the new `TimeFormat` when the user flips the 12/24-hour setting. Desktop only. */
    const TIME_FORMAT_CHANGED_EVENT = 'os-prefs://time-format-changed';
    function cmd(name, args) {
        return core.invoke(`${PREFIX}${name}`, args);
    }
    /** Reports which features work on this system, with a reason for each that does not or is degraded. */
    function getStatus() {
        return cmd('get_status');
    }
    /**
     * Reads the user's 12/24-hour clock preference. `source` says where the answer came from; the
     * value `default` means nothing could be read and `is24Hour` is only a guess.
     */
    function getTimeFormat() {
        return cmd('get_time_format');
    }
    /** Android's Developer Options "Animator duration scale" (default 1). Desktop and iOS always report 1. */
    function getAnimatorDurationScale() {
        return cmd('get_animator_duration_scale');
    }
    /** Opens the OS notification settings screen for this app. Android only; a no-op elsewhere. */
    async function openNotificationSettings() {
        await cmd('open_notification_settings');
    }
    /**
     * Subscribes to changes of the 12/24-hour setting and resolves to a function that unsubscribes.
     * Events are pushed on GNOME, Cinnamon, Windows and macOS; `getStatus()` reports whether this
     * system does (the `timeFormatWatch` feature).
     */
    function onTimeFormatChanged(callback) {
        return event.listen(TIME_FORMAT_CHANGED_EVENT, (event) => callback(event.payload));
    }
    /**
     * The `hourCycle` to give `Intl.DateTimeFormat` for a reading: `h23` for 24-hour and `h12` for
     * 12-hour. A guessed reading (`source` of `default`) gives `undefined`, which leaves the choice to
     * `Intl` and the locale.
     */
    function hourCycleOf(format) {
        if (format.source === 'default') {
            return undefined;
        }
        return format.is24Hour ? 'h23' : 'h12';
    }

    exports.TIME_FORMAT_CHANGED_EVENT = TIME_FORMAT_CHANGED_EVENT;
    exports.getAnimatorDurationScale = getAnimatorDurationScale;
    exports.getStatus = getStatus;
    exports.getTimeFormat = getTimeFormat;
    exports.hourCycleOf = hourCycleOf;
    exports.onTimeFormatChanged = onTimeFormatChanged;
    exports.openNotificationSettings = openNotificationSettings;

    return exports;

})({}, __TAURI__.core, __TAURI__.event);
Object.defineProperty(window.__TAURI__, 'osPrefs', { value: __TAURI_PLUGIN_OS_PREFS__ }) }
