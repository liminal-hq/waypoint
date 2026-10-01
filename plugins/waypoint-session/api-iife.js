if ('__TAURI__' in window) {
var __TAURI_PLUGIN_WAYPOINT_SESSION__ = (function (exports, core, event) {
    'use strict';

    // Exposes typed guest-side wrappers for the waypoint-session plugin
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PREFIX = 'plugin:waypoint-session|';
    const EVENT = 'waypoint-session://event';
    function cmd(name, args) {
        return core.invoke(`${PREFIX}${name}`, args);
    }
    /** Reports whether the session plugin works and which features it offers. */
    function getStatus() {
        return cmd('get_status');
    }
    /** The calling window's whole session at its current revision. */
    function getSnapshot() {
        return cmd('get_snapshot');
    }
    /** Opens a tab after `after` (or at the end) and returns its id. */
    function openTab(location, after, activate = true) {
        return cmd('open_tab', { location, after: after ?? null, activate });
    }
    function closeTab(tab) {
        return cmd('close_tab', { tab });
    }
    function activateTab(tab) {
        return cmd('activate_tab', { tab });
    }
    function moveTab(tab, index) {
        return cmd('move_tab', { tab, index });
    }
    /** Goes somewhere new in a tab: the current location joins its back history. */
    function navigate(tab, location) {
        return cmd('navigate', { tab, location });
    }
    function back(tab) {
        return cmd('back', { tab });
    }
    function forward(tab) {
        return cmd('forward', { tab });
    }
    // Tabs.
    /** Pins or unpins a tab; a tab in a pair or group carries the whole pair or group with it. */
    function pinTab(tab, pinned) {
        return cmd('pin_tab', { tab, pinned });
    }
    function setTabColour(tab, colour) {
        return cmd('set_tab_colour', { tab, colour });
    }
    function setTabHints(tab, hints) {
        return cmd('set_tab_hints', { tab, hints });
    }
    /** Reopens a closed tab (the newest when `tab` is omitted) and returns its id, or `null` when there was none. */
    function reopenTab(tab) {
        return cmd('reopen_tab', { tab: tab ?? null });
    }
    // Groups.
    /** Groups the tabs (and the rest of their pairs) and returns the new group's id. */
    function createGroup(tabs, name) {
        return cmd('create_group', { tabs, name: name ?? null });
    }
    function addToGroup(tab, group) {
        return cmd('add_to_group', { tab, group });
    }
    function removeFromGroup(tab) {
        return cmd('remove_from_group', { tab });
    }
    function renameGroup(group, name) {
        return cmd('rename_group', { group, name });
    }
    function setGroupColour(group, colour) {
        return cmd('set_group_colour', { group, colour });
    }
    function setGroupCollapsed(group, collapsed) {
        return cmd('set_group_collapsed', { group, collapsed });
    }
    /** Collapses every other group in the window. */
    function collapseOtherGroups(group) {
        return cmd('collapse_other_groups', { group });
    }
    function sortGroup(group, by) {
        return cmd('sort_group', { group, by });
    }
    /** Copies a group after itself and returns the copy's id. */
    function duplicateGroup(group) {
        return cmd('duplicate_group', { group });
    }
    function moveGroup(group, index) {
        return cmd('move_group', { group, index });
    }
    function ungroup(group) {
        return cmd('ungroup', { group });
    }
    function closeGroup(group) {
        return cmd('close_group', { group });
    }
    // Pairs.
    /** Pairs two or more tabs and returns the new pair's id. */
    function joinPair(tabs, layout) {
        return cmd('join_pair', { tabs, layout });
    }
    function separatePair(pair) {
        return cmd('separate_pair', { pair });
    }
    function setPairLayout(pair, layout) {
        return cmd('set_pair_layout', { pair, layout });
    }
    /** Sets the share of each pane, in thousandths adding up to 1000. */
    function setPairSizes(pair, sizes) {
        return cmd('set_pair_sizes', { pair, sizes });
    }
    function swapPanes(pair) {
        return cmd('swap_panes', { pair });
    }
    /** Splits a tab into a pair with a copy of itself, or undoes such a split. */
    function toggleSplit(tab) {
        return cmd('toggle_split', { tab });
    }
    // Windows.
    /** Makes a window, with a first tab at `location` when given, and returns its label. */
    function openWindow(location, geometry) {
        return cmd('open_window', { location: location ?? null, geometry: geometry ?? null });
    }
    /** Closes the calling window's session and the window, or the window named by `target`. */
    function closeWindow(target) {
        return cmd('close_window', { target: target ?? null });
    }
    function setGeometry(geometry) {
        return cmd('set_geometry', { geometry });
    }
    function setView(view) {
        return cmd('set_view', { view });
    }
    /** Moves tabs, a group or a pair out of the calling window and returns the label of the window they went to. */
    function moveTabs(what, to) {
        return cmd('move_tabs', { what, to });
    }
    /** Follows every change to the calling window's session. */
    function onTabsEvent(listener) {
        return event.listen(EVENT, (e) => listener(e.payload));
    }

    exports.activateTab = activateTab;
    exports.addToGroup = addToGroup;
    exports.back = back;
    exports.closeGroup = closeGroup;
    exports.closeTab = closeTab;
    exports.closeWindow = closeWindow;
    exports.collapseOtherGroups = collapseOtherGroups;
    exports.createGroup = createGroup;
    exports.duplicateGroup = duplicateGroup;
    exports.forward = forward;
    exports.getSnapshot = getSnapshot;
    exports.getStatus = getStatus;
    exports.joinPair = joinPair;
    exports.moveGroup = moveGroup;
    exports.moveTab = moveTab;
    exports.moveTabs = moveTabs;
    exports.navigate = navigate;
    exports.onTabsEvent = onTabsEvent;
    exports.openTab = openTab;
    exports.openWindow = openWindow;
    exports.pinTab = pinTab;
    exports.removeFromGroup = removeFromGroup;
    exports.renameGroup = renameGroup;
    exports.reopenTab = reopenTab;
    exports.separatePair = separatePair;
    exports.setGeometry = setGeometry;
    exports.setGroupCollapsed = setGroupCollapsed;
    exports.setGroupColour = setGroupColour;
    exports.setPairLayout = setPairLayout;
    exports.setPairSizes = setPairSizes;
    exports.setTabColour = setTabColour;
    exports.setTabHints = setTabHints;
    exports.setView = setView;
    exports.sortGroup = sortGroup;
    exports.swapPanes = swapPanes;
    exports.toggleSplit = toggleSplit;
    exports.ungroup = ungroup;

    return exports;

})({}, __TAURI__.core, __TAURI__.event);
Object.defineProperty(window.__TAURI__, 'waypointSession', { value: __TAURI_PLUGIN_WAYPOINT_SESSION__ }) }
