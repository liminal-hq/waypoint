// An in-memory TabsApi for building and testing the tab UI without the Rust side
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Geometry } from '@liminal-hq/waypoint-protocol/generated/Geometry';
import type { GroupId } from '@liminal-hq/waypoint-protocol/generated/GroupId';
import type { GroupSort } from '@liminal-hq/waypoint-protocol/generated/GroupSort';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { MoveTo } from '@liminal-hq/waypoint-protocol/generated/MoveTo';
import type { MoveWhat } from '@liminal-hq/waypoint-protocol/generated/MoveWhat';
import type { PairId } from '@liminal-hq/waypoint-protocol/generated/PairId';
import type { PairLayout } from '@liminal-hq/waypoint-protocol/generated/PairLayout';
import type { SessionEvent } from '@liminal-hq/waypoint-protocol/generated/SessionEvent';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import type { TabColour } from '@liminal-hq/waypoint-protocol/generated/TabColour';
import type { TabHints } from '@liminal-hq/waypoint-protocol/generated/TabHints';
import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';
import type { ViewPrefs } from '@liminal-hq/waypoint-protocol/generated/ViewPrefs';
import { FakeTabsStore, type FakeCommand, type Outcome } from './fakeTabsStore';
import type { OpenTabOptions, TabsApi } from './tabsApi';
import type { Unsubscribe } from './vfsClient';

/**
 * One window's handle on a fake session store, with the plugin's commands. The semantics are the
 * `waypoint-session` store's (`FakeTabsStore` is its TypeScript twin; the shared scripted
 * scenarios keep the two honest): the first tab is always active, closing the active tab
 * activates the most recently used tab or else the neighbour, commands that change nothing make
 * no events, every event takes the next global revision, and an error rejects with the same
 * message as Rust.
 *
 * `new FakeTabsApi()` is the milestone 2 single window (`main-1`, empty at revision 0, kept when
 * its last tab closes). To model several windows, share one `FakeTabsStore`:
 * `const store = new FakeTabsStore(); const one = new FakeTabsApi(store, 'main-1');` registers
 * the window like the plugin does on a window's first call, and a window that a command created
 * (`openWindow`, `moveTabs` to a new window) is attached with `new FakeTabsApi(store, label)`.
 */
export class FakeTabsApi implements TabsApi {
	readonly store: FakeTabsStore;
	readonly label: string;

	constructor(store: FakeTabsStore = FakeTabsStore.singleWindow(), label = 'main-1') {
		this.store = store;
		this.label = label;
		if (!store.windowLabels().includes(label) && label.startsWith('main-')) {
			store.dispatch(label, { kind: 'registerWindow', label });
		}
	}

	/** Runs a command for this window. */
	private run(command: FakeCommand): Outcome {
		return this.store.dispatch(this.label, command);
	}

	private mine(outcome: Outcome): SessionEvent[] {
		return outcome.events.filter((e) => e.window === this.label).map((e) => e.event);
	}

	private fromEvents<T>(
		outcome: Outcome,
		what: string,
		pick: (e: SessionEvent) => T | undefined,
	): T {
		for (const event of this.mine(outcome)) {
			const found = pick(event);
			if (found !== undefined) return found;
		}
		throw `internal error: ${what} produced no event`;
	}

	async getSnapshot(): Promise<SessionSnapshot> {
		return this.store.snapshot(this.label);
	}

	async openTab(location: Location, options: OpenTabOptions = {}): Promise<TabId> {
		const outcome = this.run({
			kind: 'open',
			location,
			after: options.after ?? null,
			activate: options.activate ?? true,
		});
		return this.fromEvents(outcome, 'opening a tab', (e) =>
			e.kind === 'tabOpened' ? e.tab.id : undefined,
		);
	}

	async closeTab(tab: TabId): Promise<void> {
		this.run({ kind: 'close', tab });
	}

	async activateTab(tab: TabId): Promise<void> {
		this.run({ kind: 'activate', tab });
	}

	async moveTab(tab: TabId, index: number): Promise<void> {
		this.run({ kind: 'move', tab, index });
	}

	async navigate(tab: TabId, location: Location): Promise<void> {
		this.run({ kind: 'navigate', tab, location });
	}

	async back(tab: TabId): Promise<void> {
		this.run({ kind: 'back', tab });
	}

	async forward(tab: TabId): Promise<void> {
		this.run({ kind: 'forward', tab });
	}

	async pinTab(tab: TabId, pinned: boolean): Promise<void> {
		this.run({ kind: 'pin', tab, pinned });
	}

	async setTabColour(tab: TabId, colour: TabColour | null): Promise<void> {
		this.run({ kind: 'setColour', tab, colour });
	}

	async setTabHints(tab: TabId, hints: TabHints): Promise<void> {
		this.run({ kind: 'setHints', tab, hints });
	}

	async reopenTab(tab?: TabId): Promise<TabId | null> {
		const outcome = this.run({ kind: 'reopen', tab: tab ?? null });
		return (
			outcome.events.flatMap((e) => (e.event.kind === 'tabReopened' ? [e.event.tab.id] : []))[0] ??
			null
		);
	}

	async createGroup(tabs: TabId[], name?: string): Promise<GroupId> {
		const outcome = this.run({ kind: 'createGroup', tabs, name: name ?? null });
		return this.fromEvents(outcome, 'creating a group', (e) =>
			e.kind === 'groupCreated' ? e.group.id : undefined,
		);
	}

	async addToGroup(tab: TabId, group: GroupId): Promise<void> {
		this.run({ kind: 'addToGroup', tab, group });
	}

	async removeFromGroup(tab: TabId): Promise<void> {
		this.run({ kind: 'removeFromGroup', tab });
	}

	async renameGroup(group: GroupId, name: string): Promise<void> {
		this.run({ kind: 'renameGroup', group, name });
	}

	async setGroupColour(group: GroupId, colour: TabColour | null): Promise<void> {
		this.run({ kind: 'setGroupColour', group, colour });
	}

	async setGroupCollapsed(group: GroupId, collapsed: boolean): Promise<void> {
		this.run({ kind: 'setGroupCollapsed', group, collapsed });
	}

	async collapseOtherGroups(group: GroupId): Promise<void> {
		this.run({ kind: 'collapseOthers', group });
	}

	async sortGroup(group: GroupId, by: GroupSort): Promise<void> {
		this.run({ kind: 'sortGroup', group, by });
	}

	async duplicateGroup(group: GroupId): Promise<GroupId> {
		const outcome = this.run({ kind: 'duplicateGroup', group });
		return this.fromEvents(outcome, 'duplicating a group', (e) =>
			e.kind === 'groupCreated' ? e.group.id : undefined,
		);
	}

	async moveGroup(group: GroupId, index: number): Promise<void> {
		this.run({ kind: 'moveGroup', group, index });
	}

	async ungroup(group: GroupId): Promise<void> {
		this.run({ kind: 'ungroup', group });
	}

	async closeGroup(group: GroupId): Promise<void> {
		this.run({ kind: 'closeGroup', group });
	}

	async joinPair(tabs: TabId[], layout: PairLayout): Promise<PairId> {
		const outcome = this.run({ kind: 'joinPair', tabs, layout });
		return this.fromEvents(outcome, 'joining a pair', (e) =>
			e.kind === 'pairCreated' ? e.pair.id : undefined,
		);
	}

	async separatePair(pair: PairId): Promise<void> {
		this.run({ kind: 'separatePair', pair });
	}

	async setPairLayout(pair: PairId, layout: PairLayout): Promise<void> {
		this.run({ kind: 'setPairLayout', pair, layout });
	}

	async setPairSizes(pair: PairId, sizes: number[]): Promise<void> {
		this.run({ kind: 'setPairSizes', pair, sizes });
	}

	async swapPanes(pair: PairId): Promise<void> {
		this.run({ kind: 'swapPanes', pair });
	}

	async toggleSplit(tab: TabId): Promise<void> {
		this.run({ kind: 'toggleSplit', tab });
	}

	async openWindow(location?: Location, geometry?: Geometry): Promise<string> {
		const outcome = this.run({
			kind: 'openWindow',
			location: location ?? null,
			geometry: geometry ?? null,
		});
		const opened = outcome.events.flatMap((e) =>
			e.event.kind === 'windowOpened' ? [e.event.window] : [],
		)[0];
		if (opened === undefined) throw 'internal error: opening a window produced no event';
		return opened;
	}

	async closeWindow(target?: string): Promise<void> {
		const label = target ?? this.label;
		if (!this.store.windowLabels().includes(label)) throw `no such window: ${label}`;
		this.store.dispatch(label, { kind: 'closeWindow' });
	}

	async setGeometry(geometry: Geometry): Promise<void> {
		this.run({ kind: 'setGeometry', geometry });
	}

	async setView(view: ViewPrefs): Promise<void> {
		this.run({ kind: 'setView', view });
	}

	async moveTabs(what: MoveWhat, to: MoveTo): Promise<string> {
		if (to.kind === 'newWindow' && to.label !== null) {
			// A window's label is the store's to allocate; a caller cannot pick one.
			throw `internal error: a new window cannot be given the label \`${to.label}\``;
		}
		const outcome = this.run({ kind: 'moveTabs', what, to });
		if (to.kind === 'existingWindow') return to.label;
		const opened = outcome.events.flatMap((e) =>
			e.event.kind === 'windowOpened' ? [e.event.window] : [],
		)[0];
		if (opened === undefined) throw 'internal error: moving tabs produced no target window';
		return opened;
	}

	onEvent(listener: (event: SessionEvent) => void): Unsubscribe {
		return this.store.listen(this.label, listener);
	}
}
