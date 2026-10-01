// An in-memory mirror of the `waypoint-session` store: every window's tabs, groups, pairs and view
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ClosedTab } from '@liminal-hq/waypoint-protocol/generated/ClosedTab';
import type { Geometry } from '@liminal-hq/waypoint-protocol/generated/Geometry';
import type { Group } from '@liminal-hq/waypoint-protocol/generated/Group';
import type { GroupId } from '@liminal-hq/waypoint-protocol/generated/GroupId';
import type { GroupSort } from '@liminal-hq/waypoint-protocol/generated/GroupSort';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { MoveTo } from '@liminal-hq/waypoint-protocol/generated/MoveTo';
import type { MoveWhat } from '@liminal-hq/waypoint-protocol/generated/MoveWhat';
import type { Pair } from '@liminal-hq/waypoint-protocol/generated/Pair';
import type { PairId } from '@liminal-hq/waypoint-protocol/generated/PairId';
import type { PairLayout } from '@liminal-hq/waypoint-protocol/generated/PairLayout';
import type { SessionEvent } from '@liminal-hq/waypoint-protocol/generated/SessionEvent';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import type { StoreSnapshot } from '@liminal-hq/waypoint-protocol/generated/StoreSnapshot';
import type { TabColour } from '@liminal-hq/waypoint-protocol/generated/TabColour';
import type { TabHints } from '@liminal-hq/waypoint-protocol/generated/TabHints';
import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import type { ViewPrefs } from '@liminal-hq/waypoint-protocol/generated/ViewPrefs';
import type { WindowEvent } from '@liminal-hq/waypoint-protocol/generated/WindowEvent';
import type { WindowState } from '@liminal-hq/waypoint-protocol/generated/WindowState';
import type { WindowSummary } from '@liminal-hq/waypoint-protocol/generated/WindowSummary';
import type { Handoff } from './tabsApi';

/**
 * This file is the TypeScript twin of `crates/waypoint-session` (`reducer/*.rs`, `layout.rs`,
 * `diff.rs`, `store.rs`). Rust is the source of truth: when the two disagree the fix is here, and
 * a behaviour change in the crate changes this file in the same commit. The shared scripted
 * scenarios in `crates/waypoint-session/tests/conformance/` run against both
 * (`tabsConformance.test.ts` here, `tests/conformance.rs` there).
 *
 * Known differences: ids are plain numbers and counters do not overflow; strings compare by UTF-8
 * bytes in group sorts, as Rust does.
 */

/** How many closed tabs the store remembers. */
export const CLOSED_LIMIT = 10;

export interface StorePolicy {
	/** Closing a window's last tab closes the window (the app's rule). */
	closeWindowOnLastTab: boolean;
}

/** What a caller can ask of the store; the same set as the crate's `Command`. */
export type FakeCommand =
	| { kind: 'open'; location: Location; after: TabId | null; activate: boolean }
	| { kind: 'close'; tab: TabId }
	| { kind: 'activate'; tab: TabId }
	| { kind: 'move'; tab: TabId; index: number }
	| { kind: 'navigate'; tab: TabId; location: Location }
	| { kind: 'back'; tab: TabId }
	| { kind: 'forward'; tab: TabId }
	| { kind: 'pin'; tab: TabId; pinned: boolean }
	| { kind: 'setColour'; tab: TabId; colour: TabColour | null }
	| { kind: 'setHints'; tab: TabId; hints: TabHints }
	| { kind: 'reopen'; tab: TabId | null }
	| { kind: 'createGroup'; tabs: TabId[]; name: string | null }
	| { kind: 'addToGroup'; tab: TabId; group: GroupId }
	| { kind: 'removeFromGroup'; tab: TabId }
	| { kind: 'renameGroup'; group: GroupId; name: string }
	| { kind: 'setGroupColour'; group: GroupId; colour: TabColour | null }
	| { kind: 'setGroupCollapsed'; group: GroupId; collapsed: boolean }
	| { kind: 'collapseOthers'; group: GroupId }
	| { kind: 'sortGroup'; group: GroupId; by: GroupSort }
	| { kind: 'duplicateGroup'; group: GroupId }
	| { kind: 'moveGroup'; group: GroupId; index: number }
	| { kind: 'ungroup'; group: GroupId }
	| { kind: 'closeGroup'; group: GroupId }
	| { kind: 'joinPair'; tabs: TabId[]; layout: PairLayout }
	| { kind: 'separatePair'; pair: PairId }
	| { kind: 'setPairLayout'; pair: PairId; layout: PairLayout }
	| { kind: 'setPairSizes'; pair: PairId; sizes: number[] }
	| { kind: 'swapPanes'; pair: PairId }
	| { kind: 'toggleSplit'; tab: TabId }
	| { kind: 'registerWindow'; label: string }
	| { kind: 'openWindow'; location: Location | null; geometry: Geometry | null }
	| { kind: 'closeWindow' }
	| { kind: 'setGeometry'; geometry: Geometry }
	| { kind: 'setView'; view: ViewPrefs }
	| { kind: 'moveTabs'; what: MoveWhat; to: MoveTo };

/** What one command did: the events in revision order and whether the last window went. */
export interface Outcome {
	events: WindowEvent[];
	lastWindowClosed: boolean;
}

/** An error with the message Rust's `SessionError` has; thrown as a string, as Tauri rejects. */
const unknownTab = (id: TabId) => `no such tab: ${id}`;
const unknownGroup = (id: GroupId) => `no such group: ${id}`;
const unknownPair = (id: PairId) => `no such pair: ${id}`;
const unknownWindow = (label: string) => `no such window: ${label}`;
const invalid = (why: string) => `invalid command: ${why}`;

interface State {
	revision: number;
	windows: WindowState[];
	closed: ClosedTab[];
	nextTab: number;
	nextGroup: number;
	nextPair: number;
	nextWindow: number;
}

const defaultView = (): ViewPrefs => ({ mode: 'list', showHidden: false, iconSize: 64 });

function newWindowState(label: string): WindowState {
	return {
		label,
		tabs: [],
		active: null,
		mru: [],
		groups: [],
		pairs: [],
		geometry: null,
		view: defaultView(),
	};
}

function deepEqual(a: unknown, b: unknown): boolean {
	if (a === b) return true;
	if (typeof a !== 'object' || typeof b !== 'object' || a === null || b === null) return false;
	if (Array.isArray(a) !== Array.isArray(b)) return false;
	const keysA = Object.keys(a);
	if (keysA.length !== Object.keys(b).length) return false;
	return keysA.every((k) =>
		deepEqual((a as Record<string, unknown>)[k], (b as Record<string, unknown>)[k]),
	);
}

/** The element at `index`, which the caller knows exists. */
function at<T>(items: T[], index: number): T {
	const item = items[index];
	if (item === undefined) throw new Error(`index ${index} is out of range`);
	return item;
}

/** `n` equal shares of 1000, the remainder going to the first panes. */
function equalSizes(n: number): number[] {
	if (n === 0) return [];
	const base = Math.floor(1000 / n);
	const extra = 1000 % n;
	return Array.from({ length: n }, (_, i) => base + (i < extra ? 1 : 0));
}

function newTab(id: TabId, location: Location): TabSnapshot {
	return {
		id,
		location,
		back: [],
		forward: [],
		pinned: false,
		colour: null,
		group: null,
		hints: { scrollTop: 0, focused: null },
	};
}

// Window layout (layout.rs).

const indexOf = (w: WindowState, tab: TabId): number => w.tabs.findIndex((t) => t.id === tab);
const tabOf = (w: WindowState, tab: TabId): TabSnapshot | undefined =>
	w.tabs.find((t) => t.id === tab);
const groupOf = (w: WindowState, group: GroupId): Group | undefined =>
	w.groups.find((g) => g.id === group);
const pairById = (w: WindowState, pair: PairId): Pair | undefined =>
	w.pairs.find((p) => p.id === pair);
const pairOf = (w: WindowState, tab: TabId): Pair | undefined =>
	w.pairs.find((p) => p.panes.includes(tab));

/** The tabs that must travel together with `tab`: its pair's panes in pane order, or the tab. */
/** "Group N" for a group made without a name: N counts the window's groups and skips taken numbers. */
function defaultGroupName(w: WindowState): string {
	let n = w.groups.length + 1;
	while (w.groups.some((g) => g.name === `Group ${n}`)) n += 1;
	return `Group ${n}`;
}

function unit(w: WindowState, tab: TabId): TabId[] {
	const pair = pairOf(w, tab);
	return pair ? pair.panes.filter((p) => indexOf(w, p) >= 0) : [tab];
}

const groupTabs = (w: WindowState, group: GroupId): TabId[] =>
	w.tabs.filter((t) => t.group === group).map((t) => t.id);

/**
 * The legal order nearest to the current one. Units (a pair's panes, or a single tab) keep their
 * first-seen position, a group gathers at the position of its first member, and the pinned blocks
 * move before the unpinned ones, each side keeping its relative order.
 */
function canonicalOrder(w: WindowState): TabId[] {
	const seen = new Set<TabId>();
	const blocks: { group: GroupId | null; pinned: boolean; ids: TabId[] }[] = [];
	for (const t of w.tabs) {
		if (seen.has(t.id)) continue;
		const ids = unit(w, t.id).filter((id) => {
			if (seen.has(id)) return false;
			seen.add(id);
			return true;
		});
		const lead = tabOf(w, at(ids, 0)) ?? t;
		const block = lead.group === null ? undefined : blocks.find((b) => b.group === lead.group);
		if (block) block.ids.push(...ids);
		else blocks.push({ group: lead.group, pinned: lead.pinned, ids });
	}
	return [...blocks.filter((b) => b.pinned), ...blocks.filter((b) => !b.pinned)].flatMap(
		(b) => b.ids,
	);
}

/** Everything wrong with a window, as readable lines; empty when every invariant holds. */
function windowViolations(w: WindowState): string[] {
	const out: string[] = [];
	const label = w.label;
	if (new Set(w.tabs.map((t) => t.id)).size !== w.tabs.length) {
		out.push(`${label}: tab ids are not unique`);
	}
	if (w.active === null && w.tabs.length > 0) out.push(`${label}: no active tab`);
	else if (w.active !== null && indexOf(w, w.active) < 0) {
		out.push(`${label}: the active tab is not a tab`);
	}
	if (w.mru.some((id, i) => indexOf(w, id) < 0 || w.mru.indexOf(id) !== i)) {
		out.push(`${label}: the mru list names a missing or repeated tab`);
	}
	const groupIds = new Set<GroupId>();
	for (const g of w.groups) {
		if (groupIds.has(g.id)) out.push(`${label}: group ids are not unique`);
		groupIds.add(g.id);
		const members = w.tabs.filter((t) => t.group === g.id);
		const first = members[0];
		if (!first) out.push(`${label}: group ${g.id} has no tabs`);
		else if (members.some((t) => t.pinned !== first.pinned)) {
			out.push(`${label}: group ${g.id} mixes pinned and unpinned`);
		}
	}
	for (const t of w.tabs) {
		if (t.group !== null && !groupIds.has(t.group)) {
			out.push(`${label}: tab ${t.id} is in a missing group`);
		}
	}
	const paired = new Set<TabId>();
	const pairIds = new Set<PairId>();
	for (const p of w.pairs) {
		if (pairIds.has(p.id)) out.push(`${label}: pair ids are not unique`);
		pairIds.add(p.id);
		if (p.panes.length < 2) out.push(`${label}: pair ${p.id} has fewer than two panes`);
		if (p.sizes.length !== p.panes.length || p.sizes.reduce((a, b) => a + b, 0) !== 1000) {
			out.push(`${label}: pair ${p.id} has bad sizes`);
		}
		const tabs = p.panes.map((id) => tabOf(w, id));
		if (tabs.some((t) => t === undefined)) {
			out.push(`${label}: pair ${p.id} has a missing pane`);
			continue;
		}
		const live = tabs as TabSnapshot[];
		for (const id of p.panes) {
			if (paired.has(id)) out.push(`${label}: a tab is in two pairs`);
			paired.add(id);
		}
		const lead = at(live, 0);
		if (live.some((t) => t.group !== lead.group || t.pinned !== lead.pinned)) {
			out.push(`${label}: pair ${p.id} spans groups or mixes pinned and unpinned`);
		}
		const first = indexOf(w, at(p.panes, 0));
		if (!p.panes.every((id, i) => indexOf(w, id) === first + i)) {
			out.push(`${label}: pair ${p.id} panes are not contiguous`);
		}
	}
	const canonical = canonicalOrder(w);
	if (
		!deepEqual(
			canonical,
			w.tabs.map((t) => t.id),
		)
	) {
		out.push(`${label}: tabs are not in a legal order (groups contiguous, pinned first)`);
	}
	return out;
}

// Helpers the command modules share (reducer.rs).

/** The tabs of `ids` and the rest of their pairs, in display order, each once. */
function expandUnits(w: WindowState, ids: TabId[]): TabId[] {
	const wanted = new Set<TabId>();
	for (const id of ids) {
		if (indexOf(w, id) < 0) throw unknownTab(id);
		for (const u of unit(w, id)) wanted.add(u);
	}
	return w.tabs.map((t) => t.id).filter((id) => wanted.has(id));
}

interface Removed {
	tab: TabSnapshot;
	index: number;
}

/**
 * Takes tabs out of a window and keeps its active tab and MRU list valid: when the active tab
 * goes, `prefer` (when it stays) or the first live MRU entry takes over, or else the tab that
 * takes its place, or the one before it. Returns what was removed, in display order.
 */
function removeTabs(w: WindowState, ids: Set<TabId>, prefer: TabId | null): Removed[] {
	const activeIndex = w.active === null ? -1 : indexOf(w, w.active);
	const activeGoes = w.active !== null && ids.has(w.active);
	const removed: Removed[] = [];
	const kept: TabSnapshot[] = [];
	let beforeActive = 0;
	w.tabs.forEach((tab, index) => {
		if (ids.has(tab.id)) removed.push({ tab, index });
		else {
			if (activeIndex >= 0 && index < activeIndex) beforeActive += 1;
			kept.push(tab);
		}
	});
	w.tabs = kept;
	w.mru = w.mru.filter((id) => !ids.has(id));
	if (activeGoes) {
		const live = (id: TabId | null): TabId | null =>
			id !== null && indexOf(w, id) >= 0 ? id : null;
		w.active =
			live(prefer) ??
			w.mru[0] ??
			(w.tabs[beforeActive] ?? (beforeActive > 0 ? w.tabs[beforeActive - 1] : undefined))?.id ??
			null;
	}
	return removed;
}

/**
 * Moves `block` (tabs in the order they should land) so its first tab sits at `index` among the
 * remaining tabs (clamped). When `within` names a group the block stays inside that group's run.
 */
function moveBlock(w: WindowState, block: TabId[], index: number, within: GroupId | null): void {
	const wanted = new Set(block);
	const moving: TabSnapshot[] = [];
	const rest: TabSnapshot[] = [];
	for (const tab of w.tabs) (wanted.has(tab.id) ? moving : rest).push(tab);
	moving.sort((a, b) => block.indexOf(a.id) - block.indexOf(b.id));
	let place = Math.min(index, rest.length);
	if (within !== null) {
		const positions = rest.flatMap((t, i) => (t.group === within ? [i] : []));
		const first = positions[0];
		const last = positions[positions.length - 1];
		if (first !== undefined && last !== undefined)
			place = Math.min(Math.max(place, first), last + 1);
	}
	rest.splice(place, 0, ...moving);
	w.tabs = rest;
}

/** Puts `ids` (in that order) right after `anchor`, which stays where it is. */
function gatherAfter(w: WindowState, anchor: TabId, ids: TabId[]): void {
	const wanted = new Set(ids);
	const moving: TabSnapshot[] = [];
	const rest: TabSnapshot[] = [];
	for (const tab of w.tabs) (wanted.has(tab.id) && tab.id !== anchor ? moving : rest).push(tab);
	moving.sort((a, b) => ids.indexOf(a.id) - ids.indexOf(b.id));
	const found = rest.findIndex((t) => t.id === anchor);
	rest.splice(found < 0 ? rest.length : found + 1, 0, ...moving);
	w.tabs = rest;
}

// Diff (diff.rs): the difference between two states as granular events.

const navigated = (a: TabSnapshot, b: TabSnapshot): boolean =>
	!deepEqual(a.location, b.location) ||
	!deepEqual(a.back, b.back) ||
	!deepEqual(a.forward, b.forward);

/** The `[tab, index]` moves that turn `from` into `to` (the same set of tabs). */
function reorder(from: TabId[], to: TabId[]): [TabId, number][] {
	if (deepEqual(from, to)) return [];
	for (const [i, x] of from.entries()) {
		const restFrom = from.filter((t) => t !== x);
		const restTo = to.filter((t) => t !== x);
		if (deepEqual(restFrom, restTo)) {
			const place = to.indexOf(x);
			return [[x, place < 0 ? i : place]];
		}
	}
	const moves: [TabId, number][] = [];
	const cur = [...from];
	for (const [i, want] of to.entries()) {
		if (cur[i] !== want) {
			const place = cur.indexOf(want);
			const [moved] = cur.splice(place < 0 ? i : place, 1);
			cur.splice(i, 0, moved as TabId);
			moves.push([want, i]);
		}
	}
	return moves;
}

// Revisions are stamped by the caller.
const R = 0;

function windowEvents(
	before: WindowState,
	after: WindowState,
	reopened: TabId | null,
): SessionEvent[] {
	const out: SessionEvent[] = [];
	for (const g of after.groups) {
		if (!groupOf(before, g.id)) out.push({ kind: 'groupCreated', group: g, revision: R });
	}
	for (const p of after.pairs) {
		if (!pairById(before, p.id)) out.push({ kind: 'pairCreated', pair: p, revision: R });
	}
	const current: TabId[] = [];
	for (const t of before.tabs) {
		if (indexOf(after, t.id) < 0) out.push({ kind: 'tabClosed', tab: t.id, revision: R });
		else current.push(t.id);
	}
	const target = after.tabs.map((t) => t.id);
	after.tabs.forEach((t, index) => {
		if (indexOf(before, t.id) >= 0) return;
		const place = Math.min(index, current.length);
		current.splice(place, 0, t.id);
		out.push({
			kind: reopened === t.id ? 'tabReopened' : 'tabOpened',
			tab: t,
			index: place,
			revision: R,
		});
	});
	for (const [tab, index] of reorder(current, target)) {
		out.push({ kind: 'tabMoved', tab, index, revision: R });
	}
	for (const t of after.tabs) {
		const old = tabOf(before, t.id);
		if (!old || deepEqual(old, t)) continue;
		out.push({ kind: navigated(old, t) ? 'tabNavigated' : 'tabChanged', tab: t, revision: R });
	}
	if (after.active !== before.active && after.active !== null) {
		out.push({ kind: 'tabActivated', tab: after.active, revision: R });
	}
	for (const p of after.pairs) {
		const old = pairById(before, p.id);
		if (old && !deepEqual(old, p)) out.push({ kind: 'pairChanged', pair: p, revision: R });
	}
	for (const g of after.groups) {
		const old = groupOf(before, g.id);
		if (old && !deepEqual(old, g)) out.push({ kind: 'groupChanged', group: g, revision: R });
	}
	for (const p of before.pairs) {
		if (!pairById(after, p.id)) out.push({ kind: 'pairRemoved', pair: p.id, revision: R });
	}
	for (const g of before.groups) {
		if (!groupOf(after, g.id)) out.push({ kind: 'groupRemoved', group: g.id, revision: R });
	}
	if (!deepEqual(after.mru, before.mru))
		out.push({ kind: 'mruChanged', mru: after.mru, revision: R });
	if (!deepEqual(after.view, before.view))
		out.push({ kind: 'viewChanged', view: after.view, revision: R });
	if (!deepEqual(after.geometry, before.geometry) && after.geometry !== null) {
		out.push({ kind: 'geometryChanged', geometry: after.geometry, revision: R });
	}
	return out;
}

function storeEvents(before: State, after: State, reopened: TabId | null): WindowEvent[] {
	const out: WindowEvent[] = [];
	const push = (window: string, event: SessionEvent) => out.push({ window, event });
	for (const old of before.windows) {
		const now = after.windows.find((w) => w.label === old.label);
		if (now) {
			for (const event of windowEvents(old, now, reopened)) push(old.label, event);
		} else {
			for (const tab of old.tabs) push(old.label, { kind: 'tabClosed', tab: tab.id, revision: R });
			push(old.label, { kind: 'windowClosed', window: old.label, revision: R });
		}
	}
	for (const now of after.windows) {
		if (before.windows.some((w) => w.label === now.label)) continue;
		push(now.label, { kind: 'windowOpened', window: now.label, revision: R });
		for (const event of windowEvents(newWindowState(now.label), now, reopened)) {
			push(now.label, event);
		}
	}
	return out;
}

// The store.

/** Another window's pair or group is a single block; this is its stable sort key. */
function sortKey(tab: TabSnapshot, by: GroupSort): [boolean, string] {
	const name = () => {
		const display = tab.location.display.replace(/[/\\]+$/, '');
		return (display.split(/[/\\]/).pop() ?? display).toLowerCase();
	};
	switch (by) {
		case 'name':
			return [false, name()];
		case 'location':
			return [false, tab.location.uri];
		case 'localFirst':
			return [!tab.location.uri.startsWith('file://'), name()];
	}
}

const encoder = new TextEncoder();

/** Compares strings by their UTF-8 bytes, as Rust's `str` ordering does. */
function compareBytes(a: string, b: string): number {
	const x = encoder.encode(a);
	const y = encoder.encode(b);
	for (let i = 0; i < Math.min(x.length, y.length); i += 1) {
		const d = (x[i] as number) - (y[i] as number);
		if (d !== 0) return d;
	}
	return x.length - y.length;
}

/** The last component of a display path, which is what a tab is titled with (as `folder_name` in Rust). */
function folderName(display: string): string {
	const trimmed = display.replace(/[/\\]+$/, '');
	const name = trimmed.split(/[/\\]/).pop();
	return name ? name : display;
}

export interface FakeTabsStoreOptions {
	policy?: StorePolicy;
	/**
	 * Makes the webview for a window a command created (`openWindow`, `moveTabs` to a new window).
	 * It runs after the change, before any event is delivered; throwing undoes the change, as a
	 * failing `WindowFactory` does in the plugin.
	 */
	createWindow?: (label: string, geometry: Geometry | null) => void;
	/** Runs when the last window closes. */
	onLastWindowClosed?: () => void;
}

type Listener = (event: SessionEvent) => void;

/**
 * Every window of a fake app: plain data with one writer. `dispatch` applies a command to a copy
 * and replaces the state only when it succeeds, so an error changes nothing, then delivers each
 * event to the listeners of the window it belongs to, in revision order.
 */
export class FakeTabsStore {
	private state: State = {
		revision: 0,
		windows: [],
		closed: [],
		nextTab: 1,
		nextGroup: 1,
		nextPair: 1,
		nextWindow: 1,
	};
	private readonly listeners = new Map<string, Set<Listener>>();
	private readonly handoffListeners = new Map<string, Set<(handoff: Handoff) => void>>();
	readonly policy: StorePolicy;
	private readonly options: FakeTabsStoreOptions;

	constructor(options: FakeTabsStoreOptions = {}) {
		this.options = options;
		this.policy = options.policy ?? { closeWindowOnLastTab: true };
	}

	/**
	 * The milestone 2 view: one window, `main-1`, already there at revision 0, which stays when
	 * its last tab closes. It is what `new FakeTabsApi()` uses, like the crate's `Session`.
	 */
	static singleWindow(options: FakeTabsStoreOptions = {}): FakeTabsStore {
		const store = new FakeTabsStore({ policy: { closeWindowOnLastTab: false }, ...options });
		store.state.windows.push(newWindowState('main-1'));
		store.state.nextWindow = 2;
		return store;
	}

	get revision(): number {
		return this.state.revision;
	}

	windowLabels(): string[] {
		return this.state.windows.map((w) => w.label);
	}

	window(label: string): WindowState | undefined {
		const w = this.state.windows.find((x) => x.label === label);
		return w ? structuredClone(w) : undefined;
	}

	/** Recently closed tabs, newest first. */
	closed(): ClosedTab[] {
		return structuredClone(this.state.closed);
	}

	/** One window's session at the current revision. */
	snapshot(label: string): SessionSnapshot {
		const w = this.state.windows.find((x) => x.label === label);
		if (!w) throw unknownWindow(label);
		return structuredClone({
			revision: this.state.revision,
			tabs: w.tabs,
			active: w.active,
			mru: w.mru,
			groups: w.groups,
			pairs: w.pairs,
			geometry: w.geometry,
			view: w.view,
			closed: this.state.closed,
		});
	}

	/** The whole store as plain data, as the crate's `to_snapshot`. */
	toSnapshot(): StoreSnapshot {
		return structuredClone({
			revision: this.state.revision,
			windows: this.state.windows,
			closed: this.state.closed,
			nextTab: this.state.nextTab,
			nextGroup: this.state.nextGroup,
			nextPair: this.state.nextPair,
			nextWindow: this.state.nextWindow,
		});
	}

	/** Everything wrong with the store, as readable lines; empty when every invariant holds. */
	violations(): string[] {
		const s = this.state;
		const out: string[] = [];
		const labels = new Set<string>();
		const tabs = new Set<TabId>();
		const groups = new Set<GroupId>();
		const pairs = new Set<PairId>();
		for (const w of s.windows) {
			if (labels.has(w.label)) out.push(`${w.label}: window label is not unique`);
			labels.add(w.label);
			out.push(...windowViolations(w));
			for (const t of w.tabs) {
				if (tabs.has(t.id)) out.push(`tab ${t.id} is in two windows`);
				tabs.add(t.id);
				if (t.id >= s.nextTab) out.push(`tab ${t.id} is at or above next_tab`);
			}
			for (const g of w.groups) {
				if (groups.has(g.id)) out.push(`group ${g.id} is in two windows`);
				groups.add(g.id);
				if (g.id >= s.nextGroup) out.push(`group ${g.id} is at or above next_group`);
			}
			for (const p of w.pairs) {
				if (pairs.has(p.id)) out.push(`pair ${p.id} is in two windows`);
				pairs.add(p.id);
				if (p.id >= s.nextPair) out.push(`pair ${p.id} is at or above next_pair`);
			}
		}
		if (s.closed.length > CLOSED_LIMIT) out.push('too many closed tabs');
		for (const c of s.closed) {
			if (tabs.has(c.tab.id)) out.push(`closed tab ${c.tab.id} is also open`);
			if (c.tab.id >= s.nextTab) out.push(`closed tab ${c.tab.id} is at or above next_tab`);
		}
		return out;
	}

	/** Follows the events of one window. A window without a webview has no listener, so its events go nowhere. */
	listen(label: string, listener: Listener): () => void {
		let set = this.listeners.get(label);
		if (!set) this.listeners.set(label, (set = new Set()));
		set.add(listener);
		return () => {
			set.delete(listener);
		};
	}

	/** Every window as a menu lists it: titled by its active folder, `caller` marked. */
	windowSummaries(caller: string): WindowSummary[] {
		return this.state.windows.map((w) => {
			const shown = w.tabs.find((t) => t.id === w.active) ?? w.tabs[0];
			return {
				label: w.label,
				title: shown ? folderName(shown.location.display) : '',
				tabCount: w.tabs.length,
				active: w.label === caller,
			};
		});
	}

	/** Follows the tabs handed to one window (the plugin's `HANDOFF_EVENT`). */
	listenHandoff(label: string, listener: (handoff: Handoff) => void): () => void {
		let set = this.handoffListeners.get(label);
		if (!set) this.handoffListeners.set(label, (set = new Set()));
		set.add(listener);
		return () => {
			set.delete(listener);
		};
	}

	/** Tells the window `label` that `handoff` arrived; what `move_tabs` does after a move into an existing window. */
	notifyHandoff(label: string, handoff: Handoff): void {
		for (const listener of [...(this.handoffListeners.get(label) ?? [])]) listener(handoff);
	}

	/** A window that went away (its webview was destroyed): its session closes. */
	destroyWindow(label: string): void {
		if (this.state.windows.some((w) => w.label === label))
			this.dispatch(label, { kind: 'closeWindow' });
	}

	/**
	 * Applies a command for the window `label`. On an error (a thrown string, as Rust's message)
	 * nothing changes. A command that changes nothing makes no events and keeps the revision.
	 */
	dispatch(label: string, command: FakeCommand): Outcome {
		const before = this.state;
		const next = structuredClone(before);
		const facts = { reopened: null as TabId | null };
		new Reducer(next, this.policy, label, facts).apply(command);
		settle(next);
		const events = storeEvents(before, next, facts.reopened);
		for (const e of events) {
			next.revision += 1;
			e.event.revision = next.revision;
		}
		const lastWindowClosed = before.windows.length > 0 && next.windows.length === 0;
		const outcome: Outcome = { events, lastWindowClosed };
		this.state = next;

		const opened = events.flatMap((e) => (e.event.kind === 'windowOpened' ? [e.event.window] : []));
		if (command.kind !== 'registerWindow') {
			try {
				for (const opening of opened) {
					const geometry = next.windows.find((w) => w.label === opening)?.geometry ?? null;
					this.options.createWindow?.(opening, geometry);
				}
			} catch (e) {
				this.state = before;
				throw `could not create the window: ${String(e)}`;
			}
		}
		for (const e of events) {
			for (const listener of [...(this.listeners.get(e.window) ?? [])])
				listener(structuredClone(e.event));
		}
		if (lastWindowClosed) this.options.onLastWindowClosed?.();
		return outcome;
	}
}

/** Brings every window into a legal shape after a command: prunes what dangles, then orders. */
function settle(s: State): void {
	for (const w of s.windows) {
		const live = new Set(w.tabs.map((t) => t.id));
		// Panes that no longer exist leave their pair; a pair of fewer than two is gone.
		for (const p of w.pairs) {
			if (p.panes.some((id) => !live.has(id))) {
				p.panes = p.panes.filter((id) => live.has(id));
				p.sizes = equalSizes(p.panes.length);
			}
		}
		w.pairs = w.pairs.filter((p) => p.panes.length >= 2);
		const groupIds = new Set(w.groups.map((g) => g.id));
		for (const t of w.tabs) {
			if (t.group !== null && !groupIds.has(t.group)) t.group = null;
		}
		const used = new Set(w.tabs.flatMap((t) => (t.group === null ? [] : [t.group])));
		w.groups = w.groups.filter((g) => used.has(g.id));
		w.mru = w.mru.filter((id) => live.has(id));
		if (w.active === null || !live.has(w.active)) w.active = w.tabs[0]?.id ?? null;
		const order = canonicalOrder(w);
		if (
			!deepEqual(
				order,
				w.tabs.map((t) => t.id),
			)
		) {
			const byId = new Map(w.tabs.map((t) => [t.id, t]));
			w.tabs = order.flatMap((id) => {
				const t = byId.get(id);
				return t ? [t] : [];
			});
		}
	}
}

/** The command modules (reducer/{tabs,groups,pairs,windows}.rs) over one copy of the state. */
class Reducer {
	constructor(
		private readonly s: State,
		private readonly policy: StorePolicy,
		private readonly label: string,
		private readonly facts: { reopened: TabId | null },
	) {}

	private win(label = this.label): WindowState {
		const w = this.s.windows.find((x) => x.label === label);
		if (!w) throw unknownWindow(label);
		return w;
	}

	private tab(w: WindowState, id: TabId): TabSnapshot {
		const t = tabOf(w, id);
		if (!t) throw unknownTab(id);
		return t;
	}

	private group(w: WindowState, id: GroupId): Group {
		const g = groupOf(w, id);
		if (!g) throw unknownGroup(id);
		return g;
	}

	private pair(w: WindowState, id: PairId): Pair {
		const p = pairById(w, id);
		if (!p) throw unknownPair(id);
		return p;
	}

	apply(c: FakeCommand): void {
		switch (c.kind) {
			case 'registerWindow':
			case 'openWindow':
			case 'closeWindow':
			case 'setGeometry':
			case 'setView':
			case 'moveTabs':
				return this.windows(c);
			case 'createGroup':
			case 'addToGroup':
			case 'removeFromGroup':
			case 'renameGroup':
			case 'setGroupColour':
			case 'setGroupCollapsed':
			case 'collapseOthers':
			case 'sortGroup':
			case 'duplicateGroup':
			case 'moveGroup':
			case 'ungroup':
			case 'closeGroup':
				return this.groups(c);
			case 'joinPair':
			case 'separatePair':
			case 'setPairLayout':
			case 'setPairSizes':
			case 'swapPanes':
			case 'toggleSplit':
				return this.pairs(c);
			default:
				return this.tabs(c);
		}
	}

	// Tabs (reducer/tabs.rs).

	private tabs(c: FakeCommand): void {
		const s = this.s;
		const w = this.win();
		switch (c.kind) {
			case 'open': {
				const id = s.nextTab;
				let index = w.tabs.length;
				let group: GroupId | null = null;
				let pinned = false;
				if (c.after !== null) {
					const anchor = this.tab(w, c.after);
					const end = Math.max(0, ...unit(w, c.after).map((u) => indexOf(w, u)));
					index = end + 1;
					// A pinned tab outside any group does not pass its pin to a new tab.
					group = anchor.group;
					pinned = group !== null && anchor.pinned;
				}
				w.tabs.splice(index, 0, { ...newTab(id, c.location), pinned, group });
				if (c.activate || w.active === null) w.active = id;
				s.nextTab += 1;
				return;
			}
			case 'close':
				this.tab(w, c.tab);
				this.closeTabs(w, new Set([c.tab]), true, null);
				return;
			case 'activate':
				this.tab(w, c.tab);
				if (w.active !== c.tab) {
					w.active = c.tab;
					w.mru = [c.tab, ...w.mru.filter((t) => t !== c.tab)];
				}
				return;
			case 'move': {
				const group = this.tab(w, c.tab).group;
				moveBlock(w, unit(w, c.tab), c.index, group);
				return;
			}
			case 'navigate': {
				const t = this.tab(w, c.tab);
				if (!deepEqual(t.location, c.location)) {
					t.back.push(t.location);
					t.location = c.location;
					t.forward = [];
				}
				return;
			}
			case 'back': {
				const t = this.tab(w, c.tab);
				const previous = t.back.pop();
				if (previous !== undefined) {
					t.forward.push(t.location);
					t.location = previous;
				}
				return;
			}
			case 'forward': {
				const t = this.tab(w, c.tab);
				const following = t.forward.pop();
				if (following !== undefined) {
					t.back.push(t.location);
					t.location = following;
				}
				return;
			}
			case 'pin': {
				const group = this.tab(w, c.tab).group;
				const set = new Set(group !== null ? groupTabs(w, group) : unit(w, c.tab));
				for (const t of w.tabs) if (set.has(t.id)) t.pinned = c.pinned;
				return;
			}
			case 'setColour': {
				// A pair shares one colour, so colouring one half colours both.
				this.tab(w, c.tab);
				const set = new Set(unit(w, c.tab));
				for (const t of w.tabs) if (set.has(t.id)) t.colour = c.colour;
				return;
			}
			case 'setHints':
				this.tab(w, c.tab).hints = c.hints;
				return;
			case 'reopen':
				return this.reopen(c.tab);
			default:
				throw new Error(`routed elsewhere: ${c.kind}`);
		}
	}

	/**
	 * Removes tabs from a window, recording them as closed when `record` is set, and closes the
	 * window when it is left empty and the policy says so.
	 */
	private closeTabs(w: WindowState, ids: Set<TabId>, record: boolean, prefer: TabId | null): void {
		const removed = removeTabs(w, ids, prefer);
		if (record) {
			for (const r of removed) {
				this.s.closed.unshift({ tab: r.tab, window: w.label, index: r.index });
			}
			this.s.closed.length = Math.min(this.s.closed.length, CLOSED_LIMIT);
		}
		if (w.tabs.length === 0 && this.policy.closeWindowOnLastTab) {
			this.s.windows = this.s.windows.filter((x) => x !== w);
		}
	}

	private reopen(tab: TabId | null): void {
		const s = this.s;
		let position: number;
		if (tab === null) {
			if (s.closed.length === 0) return;
			position = 0;
		} else {
			position = s.closed.findIndex((c) => c.tab.id === tab);
			if (position < 0) throw unknownTab(tab);
		}
		const [entry] = s.closed.splice(position, 1) as [ClosedTab];
		const w = s.windows.find((x) => x.label === entry.window) ?? this.win();
		const reopened = entry.tab;
		const index = Math.min(entry.index, w.tabs.length);
		// The old group is kept only when the tab goes back inside or next to its run.
		if (reopened.group !== null) {
			const members = w.tabs.flatMap((t, i) => (t.group === reopened.group ? [i] : []));
			const first = members[0];
			const last = members[members.length - 1];
			const keep =
				first !== undefined &&
				last !== undefined &&
				first <= index &&
				index <= last + 1 &&
				at(w.tabs, first).pinned === reopened.pinned;
			if (!keep) reopened.group = null;
		}
		w.tabs.splice(index, 0, reopened);
		w.active = reopened.id;
		w.mru = [reopened.id, ...w.mru.filter((t) => t !== reopened.id)];
		this.facts.reopened = reopened.id;
	}

	// Groups (reducer/groups.rs).

	private groups(c: FakeCommand): void {
		const s = this.s;
		const w = this.win();
		switch (c.kind) {
			case 'createGroup': {
				if (c.tabs.length === 0) throw invalid('a group needs at least one tab');
				const id = s.nextGroup;
				const members = expandUnits(w, c.tabs);
				const pinned = tabOf(w, at(members, 0))?.pinned ?? false;
				for (const t of w.tabs) {
					if (members.includes(t.id)) {
						t.group = id;
						t.pinned = pinned;
					}
				}
				w.groups.push({ id, name: c.name ?? defaultGroupName(w), colour: null, collapsed: false });
				s.nextGroup += 1;
				return;
			}
			case 'addToGroup': {
				const lead = this.tab(w, c.tab);
				this.group(w, c.group);
				if (lead.group === c.group) return;
				const moving = unit(w, c.tab);
				const members = groupTabs(w, c.group);
				const first = members[0];
				const pinned = first === undefined ? false : (tabOf(w, first)?.pinned ?? false);
				for (const t of w.tabs) {
					if (moving.includes(t.id)) {
						t.group = c.group;
						t.pinned = pinned;
					}
				}
				const last = members[members.length - 1];
				if (last !== undefined) gatherAfter(w, last, moving);
				return;
			}
			case 'removeFromGroup': {
				this.tab(w, c.tab);
				const moving = unit(w, c.tab);
				for (const t of w.tabs) if (moving.includes(t.id)) t.group = null;
				return;
			}
			case 'renameGroup':
				this.group(w, c.group).name = c.name;
				return;
			case 'setGroupColour':
				this.group(w, c.group).colour = c.colour;
				return;
			case 'setGroupCollapsed':
				this.group(w, c.group).collapsed = c.collapsed;
				return;
			case 'collapseOthers':
				this.group(w, c.group);
				for (const g of w.groups) if (g.id !== c.group) g.collapsed = true;
				return;
			case 'sortGroup':
				this.group(w, c.group);
				this.sortGroup(w, c.group, c.by);
				return;
			case 'duplicateGroup':
				return this.duplicate(w, c.group);
			case 'moveGroup':
				this.group(w, c.group);
				moveBlock(w, groupTabs(w, c.group), c.index, null);
				return;
			case 'ungroup':
				this.group(w, c.group);
				for (const t of w.tabs) if (t.group === c.group) t.group = null;
				return;
			case 'closeGroup':
				this.group(w, c.group);
				this.closeTabs(w, new Set(groupTabs(w, c.group)), true, null);
				return;
			default:
				throw new Error(`routed elsewhere: ${c.kind}`);
		}
	}

	/** Reorders a group's tabs in place, keeping each pair's panes together (a pair sorts by its first pane). */
	private sortGroup(w: WindowState, group: GroupId, by: GroupSort): void {
		const members = groupTabs(w, group);
		const units: TabId[][] = [];
		const seen = new Set<TabId>();
		for (const id of members) {
			if (seen.has(id)) continue;
			const u = unit(w, id);
			for (const x of u) seen.add(x);
			units.push(u);
		}
		const keyed = units.map((u) => ({
			u,
			key: sortKey(tabOf(w, at(u, 0)) as TabSnapshot, by),
		}));
		keyed.sort((a, b) =>
			a.key[0] === b.key[0]
				? compareBytes(a.key[1], b.key[1])
				: Number(a.key[0]) - Number(b.key[0]),
		);
		const first = indexOf(w, at(members, 0));
		moveBlock(
			w,
			keyed.flatMap((k) => k.u),
			first,
			null,
		);
	}

	private duplicate(w: WindowState, group: GroupId): void {
		const s = this.s;
		this.group(w, group);
		const newGroup = s.nextGroup;
		s.nextGroup += 1;
		const members = groupTabs(w, group);
		const ids = new Map<TabId, TabId>();
		for (const m of members) {
			ids.set(m, s.nextTab);
			s.nextTab += 1;
		}
		const firstPair = s.nextPair;
		const copies = members.flatMap((m) => {
			const t = tabOf(w, m);
			return t ? [{ ...structuredClone(t), id: ids.get(t.id) as TabId, group: newGroup }] : [];
		});
		const pairs: Pair[] = [];
		const seen = new Set<PairId>();
		for (const m of members) {
			const p = pairOf(w, m);
			if (p && !seen.has(p.id)) {
				seen.add(p.id);
				pairs.push({
					...structuredClone(p),
					id: firstPair + pairs.length,
					panes: p.panes.flatMap((x) => {
						const mapped = ids.get(x);
						return mapped === undefined ? [] : [mapped];
					}),
					origin: { kind: 'joined' },
				});
			}
		}
		s.nextPair += pairs.length;
		const source = groupOf(w, group);
		if (source) w.groups.push({ ...structuredClone(source), id: newGroup, collapsed: false });
		w.pairs.push(...pairs);
		const last = members[members.length - 1];
		const lastIndex = last === undefined ? -1 : indexOf(w, last);
		const end = lastIndex < 0 ? w.tabs.length : lastIndex + 1;
		w.tabs.splice(end, 0, ...copies);
	}

	// Pairs (reducer/pairs.rs).

	private pairs(c: FakeCommand): void {
		const s = this.s;
		const w = this.win();
		switch (c.kind) {
			case 'joinPair': {
				const unique = c.tabs.filter((t, i) => c.tabs.indexOf(t) === i);
				if (unique.length < 2) throw invalid('a pair needs at least two tabs');
				const id = s.nextPair;
				for (const t of unique) {
					this.tab(w, t);
					if (pairOf(w, t)) throw `tab ${t} is already in a pair`;
				}
				const lead = this.tab(w, at(unique, 0));
				const { group, pinned } = lead;
				for (const t of w.tabs) {
					if (unique.includes(t.id)) {
						t.group = group;
						t.pinned = pinned;
					}
				}
				gatherAfter(w, at(unique, 0), unique.slice(1));
				w.pairs.push({
					id,
					panes: unique,
					layout: c.layout,
					sizes: equalSizes(unique.length),
					origin: { kind: 'joined' },
				});
				s.nextPair += 1;
				return;
			}
			case 'separatePair':
				this.pair(w, c.pair);
				w.pairs = w.pairs.filter((p) => p.id !== c.pair);
				return;
			case 'setPairLayout':
				this.pair(w, c.pair).layout = c.layout;
				return;
			case 'setPairSizes': {
				const p = this.pair(w, c.pair);
				if (
					c.sizes.length !== p.panes.length ||
					c.sizes.includes(0) ||
					c.sizes.reduce((a, b) => a + b, 0) !== 1000
				) {
					throw invalid('pair sizes need one positive share per pane, adding up to 1000');
				}
				p.sizes = [...c.sizes];
				return;
			}
			case 'swapPanes': {
				const p = this.pair(w, c.pair);
				p.panes.reverse();
				p.sizes.reverse();
				// The tab order follows the pane order when the state settles.
				return;
			}
			case 'toggleSplit':
				return this.toggleSplit(w, c.tab);
			default:
				throw new Error(`routed elsewhere: ${c.kind}`);
		}
	}

	private toggleSplit(w: WindowState, tab: TabId): void {
		const s = this.s;
		const lead = this.tab(w, tab);
		const existing = pairOf(w, tab);
		if (existing) {
			const origin = existing.origin;
			if (origin.kind === 'toggle' && existing.panes.includes(origin.created)) {
				// Closing the pane toggling made leaves the other one, active if this was.
				const keep = existing.panes.find((p) => p !== origin.created) ?? null;
				this.closeTabs(w, new Set([origin.created]), true, keep);
			} else {
				w.pairs = w.pairs.filter((p) => p.id !== existing.id);
			}
			return;
		}
		const id = s.nextTab;
		const pairId = s.nextPair;
		const copy: TabSnapshot = {
			id,
			location: structuredClone(lead.location),
			back: [],
			forward: [],
			pinned: lead.pinned,
			colour: lead.colour,
			group: lead.group,
			hints: { scrollTop: 0, focused: null },
		};
		const place = indexOf(w, tab);
		w.tabs.splice(place < 0 ? w.tabs.length : place + 1, 0, copy);
		w.pairs.push({
			id: pairId,
			panes: [tab, id],
			layout: 'sideBySide',
			sizes: equalSizes(2),
			origin: { kind: 'toggle', created: id },
		});
		s.nextTab += 1;
		s.nextPair += 1;
	}

	// Windows (reducer/windows.rs).

	private windows(c: FakeCommand): void {
		const s = this.s;
		switch (c.kind) {
			case 'registerWindow':
				if (!c.label.startsWith('main-')) throw invalid('not a main window label');
				this.newWindow(c.label, null);
				return;
			case 'openWindow': {
				const label = this.newWindow(null, c.geometry);
				if (c.location) {
					const w = this.win(label);
					const id = s.nextTab;
					s.nextTab += 1;
					w.tabs.push(newTab(id, c.location));
					w.active = id;
				}
				return;
			}
			case 'closeWindow': {
				const w = this.win();
				s.windows = s.windows.filter((x) => x !== w);
				w.tabs.forEach((tab, index) => {
					s.closed.unshift({ tab, window: w.label, index });
				});
				s.closed.length = Math.min(s.closed.length, CLOSED_LIMIT);
				return;
			}
			case 'setGeometry':
				this.win().geometry = c.geometry;
				return;
			case 'setView':
				this.win().view = c.view;
				return;
			case 'moveTabs':
				return this.moveTabs(c.what, c.to);
			default:
				throw new Error(`routed elsewhere: ${c.kind}`);
		}
	}

	/** Adds an empty window under `label`, or the next `main-{n}`, and returns its label. */
	private newWindow(label: string | null, geometry: Geometry | null): string {
		const s = this.s;
		let chosen: string;
		if (label !== null) {
			if (s.windows.some((w) => w.label === label))
				throw invalid('a window with that label exists');
			const n = /^main-(\d+)$/.exec(label);
			if (n) s.nextWindow = Math.max(s.nextWindow, Number(n[1]) + 1);
			chosen = label;
		} else {
			chosen = `main-${s.nextWindow}`;
			s.nextWindow += 1;
		}
		const w = newWindowState(chosen);
		w.geometry = geometry;
		s.windows.push(w);
		return chosen;
	}

	private moveTabs(what: MoveWhat, to: MoveTo): void {
		const s = this.s;
		const source = this.win();
		let moving: TabId[];
		switch (what.kind) {
			case 'tabs': {
				if (what.value.length === 0) throw invalid('nothing to move');
				for (const t of what.value) this.tab(source, t);
				const set = new Set(what.value);
				moving = source.tabs.map((t) => t.id).filter((id) => set.has(id));
				break;
			}
			case 'group':
				this.group(source, what.value);
				moving = groupTabs(source, what.value);
				break;
			case 'pair':
				moving = expandUnits(source, this.pair(source, what.value).panes);
				break;
		}
		const activeMoves = source.active !== null && moving.includes(source.active);
		const set = new Set(moving);

		// Resolve the target before changing anything, so an error leaves the store as it was.
		let targetLabel: string;
		if (to.kind === 'existingWindow') {
			if (to.label === this.label) throw invalid('the tabs are already in that window');
			this.win(to.label);
			targetLabel = to.label;
		} else {
			targetLabel = this.newWindow(to.label, to.geometry);
		}

		// Groups and pairs whose members all travel keep their identity.
		const sourceActive = source.active;
		const wholeGroups = source.groups.filter((g) => {
			const members = groupTabs(source, g.id);
			return members.length > 0 && members.every((m) => set.has(m));
		});
		const wholePairs = source.pairs.filter((p) => p.panes.every((m) => set.has(m)));
		source.groups = source.groups.filter((g) => !wholeGroups.includes(g));
		source.pairs = source.pairs.filter((p) => !wholePairs.includes(p));
		const tabs = removeTabs(source, set, null).map((r) => r.tab);
		for (const t of tabs) {
			if (t.group !== null && !wholeGroups.some((g) => g.id === t.group)) t.group = null;
		}
		if (source.tabs.length === 0 && this.policy.closeWindowOnLastTab) {
			s.windows = s.windows.filter((x) => x !== source);
		}

		const target = this.win(targetLabel);
		const index = to.kind === 'existingWindow' ? Math.min(to.index, target.tabs.length) : 0;
		const first = tabs[0]?.id ?? null;
		target.tabs.splice(index, 0, ...tabs);
		target.groups.push(...wholeGroups);
		target.pairs.push(...wholePairs);
		if (target.active === null) target.active = activeMoves ? sourceActive : first;
		else if (activeMoves) target.active = sourceActive;
	}
}
