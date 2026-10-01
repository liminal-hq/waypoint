// An in-memory TabsApi for building and testing the tab UI without the Rust side
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { SessionEvent } from '@liminal-hq/waypoint-protocol/generated/SessionEvent';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import type { OpenTabOptions, TabsApi } from './tabsApi';
import type { Unsubscribe } from './vfsClient';

/**
 * Runs the `waypoint-session` reducer's semantics in TypeScript: the first tab is always active,
 * closing the active tab activates the tab that takes its place (or the one before), commands
 * that change nothing make no events, every event takes the next revision, and an unknown tab
 * rejects with the same message as Rust. `tabsApi.test.ts` and the Rust tests pin the same
 * behaviour, so the two must change together.
 */
export class FakeTabsApi implements TabsApi {
	private tabs: TabSnapshot[] = [];
	private active: TabId | null = null;
	private revision = 0;
	private nextId = 1;
	private listeners = new Set<(event: SessionEvent) => void>();

	async getSnapshot(): Promise<SessionSnapshot> {
		// Groups, pairs, MRU, geometry and the closed list are Rust-only until the fake mirrors them.
		return structuredClone({
			revision: this.revision,
			tabs: this.tabs,
			active: this.active,
			mru: [],
			groups: [],
			pairs: [],
			geometry: null,
			view: { mode: 'list', showHidden: false, iconSize: 64 },
			closed: [],
		});
	}

	async openTab(location: Location, options: OpenTabOptions = {}): Promise<TabId> {
		const index = options.after === undefined ? this.tabs.length : this.indexOf(options.after) + 1;
		const tab: TabSnapshot = {
			id: this.nextId++,
			location,
			back: [],
			forward: [],
			pinned: false,
			colour: null,
			group: null,
			hints: { scrollTop: 0, focused: null },
		};
		this.tabs.splice(index, 0, tab);
		this.emit({ kind: 'tabOpened', tab: structuredClone(tab), index, revision: 0 });
		if ((options.activate ?? true) || this.active === null) {
			this.active = tab.id;
			this.emit({ kind: 'tabActivated', tab: tab.id, revision: 0 });
		}
		return tab.id;
	}

	async closeTab(id: TabId): Promise<void> {
		const index = this.indexOf(id);
		this.tabs.splice(index, 1);
		this.emit({ kind: 'tabClosed', tab: id, revision: 0 });
		if (this.active === id) {
			this.active = (this.tabs[index] ?? this.tabs[index - 1])?.id ?? null;
			if (this.active !== null) this.emit({ kind: 'tabActivated', tab: this.active, revision: 0 });
		}
	}

	async activateTab(id: TabId): Promise<void> {
		this.indexOf(id);
		if (this.active === id) return;
		this.active = id;
		this.emit({ kind: 'tabActivated', tab: id, revision: 0 });
	}

	async moveTab(id: TabId, index: number): Promise<void> {
		const from = this.indexOf(id);
		const to = Math.min(index, this.tabs.length - 1);
		if (from === to) return;
		this.tabs.splice(to, 0, ...this.tabs.splice(from, 1));
		this.emit({ kind: 'tabMoved', tab: id, index: to, revision: 0 });
	}

	async navigate(id: TabId, location: Location): Promise<void> {
		const tab = this.tabFor(id);
		if (tab.location.uri === location.uri && tab.location.display === location.display) return;
		tab.back.push(tab.location);
		tab.location = location;
		tab.forward = [];
		this.emitNavigated(tab);
	}

	async back(id: TabId): Promise<void> {
		const tab = this.tabFor(id);
		const previous = tab.back.pop();
		if (previous === undefined) return;
		tab.forward.push(tab.location);
		tab.location = previous;
		this.emitNavigated(tab);
	}

	async forward(id: TabId): Promise<void> {
		const tab = this.tabFor(id);
		const following = tab.forward.pop();
		if (following === undefined) return;
		tab.back.push(tab.location);
		tab.location = following;
		this.emitNavigated(tab);
	}

	onEvent(listener: (event: SessionEvent) => void): Unsubscribe {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	}

	private indexOf(id: TabId): number {
		const index = this.tabs.findIndex((t) => t.id === id);
		if (index < 0) throw `no such tab: ${id}`;
		return index;
	}

	private tabFor(id: TabId): TabSnapshot {
		const tab = this.tabs[this.indexOf(id)];
		if (!tab) throw `no such tab: ${id}`;
		return tab;
	}

	private emitNavigated(tab: TabSnapshot): void {
		this.emit({ kind: 'tabNavigated', tab: structuredClone(tab), revision: 0 });
	}

	/** Stamps the next revision on an event and delivers it. */
	private emit(event: SessionEvent): void {
		this.revision += 1;
		const stamped = { ...event, revision: this.revision } as SessionEvent;
		for (const listener of [...this.listeners]) listener(stamped);
	}
}
