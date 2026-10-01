// Verifies each tab drag outcome end to end over the engine and a fake session, without a layout engine
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { fireEvent } from '@testing-library/react';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createDragSession, type DragClock, type DragSession } from '../dnd/dragSession';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { fileLocation } from '../services/fakeVfsClient';
import type { StripMeasure } from './dragLayout';
import { REST_GROUP_MS, HOLD_SPLIT_MS } from './dragTiming';
import {
	createTabDragHandlers,
	describeTabDrag,
	type TabDragHandlers,
	type TabDragSource,
	type TabDragTarget,
	type TearOffHook,
} from './tabDrag';

class ManualClock implements DragClock {
	private now = 0;
	private next = 1;
	private timers = new Map<number, { at: number; handler: () => void }>();
	setTimeout(handler: () => void, ms: number) {
		const id = this.next++;
		this.timers.set(id, { at: this.now + ms, handler });
		return id;
	}
	clearTimeout(handle: unknown) {
		this.timers.delete(handle as number);
	}
	advance(ms: number) {
		const end = this.now + ms;
		for (;;) {
			const due = [...this.timers.entries()]
				.filter(([, timer]) => timer.at <= end)
				.sort((a, b) => a[1].at - b[1].at)[0];
			if (!due) break;
			this.timers.delete(due[0]);
			this.now = due[1].at;
			due[1].handler();
		}
		this.now = end;
	}
}

const TAB_W = 100;
const CHIP_W = 60;
const STRIP = { left: 0, top: 0, right: 2000, bottom: 30 };
const AREA = { left: 0, top: 200, right: 1000, bottom: 600 };
/** The window: wider than the area so the strip, the sidebar's side and the way out are all in reach. */
const VIEW = { width: 2000, height: 700 };

/** Lays the strip out the way the page does: a chip before its group's tabs, every tab 100 wide. */
function layout(snapshot: SessionSnapshot): StripMeasure {
	let x = 0;
	const spans: StripMeasure['spans'] = [];
	const chips: StripMeasure['chips'] = [];
	let lastGroup: number | null = null;
	for (const tab of snapshot.tabs) {
		if (tab.group !== null && tab.group !== lastGroup) {
			chips.push({ group: tab.group, left: x, right: x + CHIP_W });
			x += CHIP_W;
		}
		lastGroup = tab.group;
		spans.push({ left: x, right: x + TAB_W });
		x += TAB_W;
	}
	return { spans, chips, strip: STRIP, tablistLeft: 0, area: AREA };
}

async function setup(names: string[] = ['a', 'b', 'c', 'd', 'e'], tearOff?: TearOffHook) {
	const api = new FakeTabsApi();
	const ids: Record<string, TabId> = {};
	for (const name of names) ids[name] = await api.openTab(fileLocation(`/home/test/${name}`));
	let snapshot = await api.getSnapshot();
	const refresh = async () => {
		snapshot = await api.getSnapshot();
	};
	const clock = new ManualClock();
	const live: string[] = [];
	const renames: number[] = [];
	const session: DragSession<TabDragSource, TabDragTarget> = createDragSession<
		TabDragSource,
		TabDragTarget
	>({
		startThresholdPx: 4,
		clock,
		reducedMotion: () => false,
		announce: (text) => live.push(text),
		sameTarget: (a, b) => JSON.stringify(a) === JSON.stringify(b),
	});
	let handlers: TabDragHandlers | null = null;
	const press = (what: { tab: TabId } | { group: number }, at: { x: number; y?: number }) => {
		const source = describeTabDrag(snapshot, layout(snapshot), what)!;
		handlers = createTabDragHandlers({
			api,
			snapshot: () => snapshot,
			announce: (text) => live.push(text),
			requestRename: (group) => renames.push(group),
			tearOff,
			viewport: () => VIEW,
		});
		session.begin({ pointerId: 1, clientX: at.x, clientY: at.y ?? 15, source }, handlers);
	};
	const leftWindow = () => handlers!.leftWindow();
	const move = (x: number, y = 15) =>
		fireEvent.pointerMove(window, { clientX: x, clientY: y, pointerId: 1 });
	const up = (x: number, y = 15) =>
		fireEvent.pointerUp(window, { clientX: x, clientY: y, pointerId: 1 });
	const state = () => session.store.getState();
	/** Lets a commit that the drop started finish, then reads the session again. */
	const settle = async () => {
		await new Promise((resolve) => setTimeout(resolve, 0));
		await refresh();
	};
	return {
		api,
		ids,
		clock,
		live,
		renames,
		session,
		press,
		leftWindow,
		move,
		up,
		state,
		settle,
		refresh,
		snapshot: () => snapshot,
	};
}

afterEach(() => {
	document.documentElement.removeAttribute('style');
});

const order = (snapshot: SessionSnapshot) =>
	snapshot.tabs.map((tab) => tab.location.display.slice(-1));

describe('reorder', () => {
	it('shows one pill with the new position, slides the others and commits the move', async () => {
		const h = await setup();
		h.press({ tab: h.ids.a! }, { x: 50 });
		h.move(262);
		const state = h.state();
		expect(state.phase).toBe('dragging');
		expect(state.pill).toMatchObject({
			kind: 'move',
			text: 'Release to move a to position 3 of 5',
			announce: 'Drag: release to move a to position 3 of 5',
		});
		expect(state.target).toMatchObject({ outcome: 'reorder', to: 2 });
		const preview = (state.target as Extract<TabDragTarget, { outcome: 'reorder' }>).preview;
		// The two tabs it passed slide into the space it left, and the line marks the open slot.
		expect(preview.shifts).toEqual([
			[h.ids.b, -TAB_W],
			[h.ids.c, -TAB_W],
		]);
		expect(preview.marker).toMatchObject({ kind: 'line', left: 200 });
		h.up(262);
		await h.settle();
		expect(order(h.snapshot())).toEqual(['b', 'c', 'a', 'd', 'e']);
		expect(h.live).toContain('Moved a to position 3 of 5');
	});

	it('changes nothing when released in its own slot', async () => {
		const h = await setup();
		const move = vi.spyOn(h.api, 'moveTab');
		h.press({ tab: h.ids.c! }, { x: 250 });
		h.move(262);
		h.up(262);
		await h.settle();
		expect(move).not.toHaveBeenCalled();
	});

	it('never lets a pinned tab cross the pinned boundary, nor an unpinned one', async () => {
		const h = await setup();
		await h.api.pinTab(h.ids.a!, true);
		await h.api.pinTab(h.ids.b!, true);
		await h.refresh();
		h.press({ tab: h.ids.c! }, { x: 250 });
		h.move(1);
		expect(h.state().target).toMatchObject({ outcome: 'reorder', to: 2 });
		h.up(1);
		await h.settle();
		expect(order(h.snapshot())).toEqual(['a', 'b', 'c', 'd', 'e']);

		h.press({ tab: h.ids.a! }, { x: 50 });
		h.move(900);
		expect(h.state().target).toMatchObject({ outcome: 'reorder', to: 1 });
		h.up(900);
		await h.settle();
		expect(order(h.snapshot())).toEqual(['b', 'a', 'c', 'd', 'e']);
	});

	it('moves a pair as one unit', async () => {
		const h = await setup();
		await h.api.joinPair([h.ids.a!, h.ids.b!], 'sideBySide');
		await h.refresh();
		h.press({ tab: h.ids.b! }, { x: 150 });
		// The pair is 200 wide: its centre is at 100, so 300 px of travel passes c and d.
		h.move(450);
		expect(h.state().target).toMatchObject({ outcome: 'reorder', to: 2 });
		h.up(450);
		await h.settle();
		expect(order(h.snapshot())).toEqual(['c', 'd', 'a', 'b', 'e']);
		expect(h.snapshot().pairs[0]?.panes).toEqual([h.ids.a, h.ids.b]);
	});

	it('moves a whole group by its chip', async () => {
		const h = await setup();
		const group = await h.api.createGroup([h.ids.a!, h.ids.b!]);
		await h.refresh();
		h.press({ group }, { x: 30 });
		h.move(330);
		expect(h.state().pill?.text).toBe('Release to move group Group 1');
		h.up(330);
		await h.settle();
		expect(order(h.snapshot())).toEqual(['c', 'd', 'a', 'b', 'e']);
		expect(h.snapshot().tabs.map((tab) => tab.group)).toEqual([null, null, group, group, null]);
		expect(h.live).toContain('Moved group Group 1 to position 3 of 5');
	});
});

describe('hold to split', () => {
	it('draws a ring while the hold runs, then bridges, then splits on release', async () => {
		const h = await setup();
		h.press({ tab: h.ids.a! }, { x: 50 });
		h.move(150);
		let target = h.state().target as Extract<TabDragTarget, { outcome: 'holdSplit' }>;
		expect(target).toMatchObject({ outcome: 'holdSplit', other: h.ids.b, armed: false });
		// Until the hold completes the pill is still the reorder pill.
		expect(h.state().pill?.kind).toBe('move');
		h.clock.advance(HOLD_SPLIT_MS - 1);
		expect((h.state().target as typeof target).armed).toBe(false);
		h.clock.advance(1);
		target = h.state().target as typeof target;
		expect(target.armed).toBe(true);
		expect(h.state().pill).toMatchObject({
			kind: 'split',
			text: 'Release to split with b',
			announce: 'Drag: release to split with b',
		});
		h.up(150);
		await h.settle();
		const pair = h.snapshot().pairs[0]!;
		expect(pair.panes).toEqual([h.ids.a, h.ids.b]);
		expect(order(h.snapshot())).toEqual(['a', 'b', 'c', 'd', 'e']);
		expect(h.live).toContain('Split a and b');
	});

	it('commits a reorder, not a pair, when released before the hold completes', async () => {
		const h = await setup();
		h.press({ tab: h.ids.a! }, { x: 50 });
		h.move(150);
		expect(h.state().target).toMatchObject({ outcome: 'holdSplit', armed: false, to: 0 });
		h.clock.advance(HOLD_SPLIT_MS - 1);
		h.up(150);
		await h.settle();
		expect(h.snapshot().pairs).toHaveLength(0);
		expect(order(h.snapshot())).toEqual(['a', 'b', 'c', 'd', 'e']);
		expect(h.live).not.toContain('Split');
	});

	it('puts the dragged tab after the held one when it came from the right', async () => {
		const h = await setup();
		h.press({ tab: h.ids.c! }, { x: 250 });
		// Still inside b's body, with the dragged tab's centre not yet past b's centre.
		h.move(160);
		h.clock.advance(HOLD_SPLIT_MS);
		expect(h.state().target).toMatchObject({ outcome: 'holdSplit', armed: true, before: false });
		h.up(160);
		await h.settle();
		expect(h.snapshot().pairs[0]?.panes).toEqual([h.ids.b, h.ids.c]);
		expect(order(h.snapshot())).toEqual(['a', 'b', 'c', 'd', 'e']);
	});

	it('is dropped when the pointer leaves the tab’s body, and starts over on the next', async () => {
		const h = await setup();
		h.press({ tab: h.ids.a! }, { x: 50 });
		h.move(150);
		h.clock.advance(HOLD_SPLIT_MS - 50);
		h.move(190);
		expect(h.state().target?.outcome).toBe('reorder');
		h.clock.advance(1000);
		expect(h.state().target?.outcome).not.toBe('holdSplit');
	});

	it('is not offered between tabs that cannot join: other pins, other groups, pairs', async () => {
		const h = await setup();
		await h.api.pinTab(h.ids.a!, true);
		await h.refresh();
		h.press({ tab: h.ids.a! }, { x: 50 });
		h.move(150);
		h.clock.advance(HOLD_SPLIT_MS);
		expect(h.state().target?.outcome).not.toBe('holdSplit');
		h.session.cancel();

		const g = await setup();
		await g.api.joinPair([g.ids.b!, g.ids.c!], 'sideBySide');
		await g.refresh();
		g.press({ tab: g.ids.a! }, { x: 50 });
		g.move(150);
		g.clock.advance(HOLD_SPLIT_MS);
		expect(g.state().target?.outcome).not.toBe('holdSplit');
	});

	it('joins the held tab’s group when the dragged tab was in none', async () => {
		const h = await setup();
		const group = await h.api.createGroup([h.ids.c!, h.ids.d!]);
		await h.refresh();
		// Strip: a b [chip c d] e. Hold a over c (its body is 260..360 wide, centre 310).
		h.press({ tab: h.ids.a! }, { x: 50 });
		h.move(310);
		h.clock.advance(HOLD_SPLIT_MS);
		expect(h.state().target).toMatchObject({ outcome: 'holdSplit', other: h.ids.c, armed: true });
		h.up(310);
		await h.settle();
		expect(h.snapshot().tabs.find((tab) => tab.id === h.ids.a)?.group).toBe(group);
		expect(h.snapshot().pairs).toHaveLength(1);
	});
});

describe('rest to start a group', () => {
	it('draws a bracket after the rest and creates the group with the neighbour on release', async () => {
		const h = await setup();
		h.press({ tab: h.ids.c! }, { x: 250 });
		h.move(262);
		expect(h.state().target?.outcome).toBe('reorder');
		h.clock.advance(REST_GROUP_MS - 1);
		expect(h.state().target?.outcome).toBe('reorder');
		h.clock.advance(1);
		const target = h.state().target as Extract<TabDragTarget, { outcome: 'holdGroup' }>;
		expect(target).toMatchObject({ outcome: 'holdGroup', neighbour: h.ids.b });
		expect(target.preview.marker?.kind).toBe('bracket');
		expect(h.state().pill).toMatchObject({
			kind: 'group',
			text: 'Release to start a new group',
			announce: 'Drag: release to start a new group',
		});
		h.up(262);
		await h.settle();
		const snapshot = h.snapshot();
		expect(snapshot.groups).toHaveLength(1);
		const group = snapshot.groups[0]!;
		expect(snapshot.tabs.map((tab) => tab.group)).toEqual([null, group.id, group.id, null, null]);
		expect(order(snapshot)).toEqual(['a', 'b', 'c', 'd', 'e']);
		// The new group opens with its name ready to edit.
		expect(h.renames).toEqual([group.id]);
		expect(h.live).toContain(`Created group ${group.name}`);
	});

	it('moves the tab to the slot first when it was rested somewhere else', async () => {
		const h = await setup();
		h.press({ tab: h.ids.a! }, { x: 50 });
		h.move(262);
		h.clock.advance(REST_GROUP_MS);
		expect(h.state().target).toMatchObject({ outcome: 'holdGroup', to: 2 });
		h.up(262);
		await h.settle();
		const snapshot = h.snapshot();
		expect(order(snapshot)).toEqual(['b', 'c', 'a', 'd', 'e']);
		const group = snapshot.groups[0]!.id;
		expect(snapshot.tabs.map((tab) => tab.group)).toEqual([null, group, group, null, null]);
	});

	it('restarts when the pointer moves on, and is not offered next to a group or across a pin', async () => {
		const h = await setup();
		h.press({ tab: h.ids.c! }, { x: 250 });
		h.move(262);
		h.clock.advance(REST_GROUP_MS - 100);
		h.move(280);
		h.clock.advance(200);
		expect(h.state().target?.outcome).toBe('reorder');
		h.clock.advance(REST_GROUP_MS);
		expect(h.state().target?.outcome).toBe('holdGroup');
		// Moving again disarms it.
		h.move(300);
		expect(h.state().target?.outcome).not.toBe('holdGroup');
		h.session.cancel();

		const g = await setup();
		await g.api.createGroup([g.ids.a!, g.ids.b!]);
		await g.refresh();
		// Resting at the end of the group's span, beside its last tab, would join the neighbour away.
		g.press({ tab: g.ids.e! }, { x: 650 });
		g.move(520);
		g.clock.advance(REST_GROUP_MS);
		expect(g.state().target?.outcome).not.toBe('holdGroup');
	});
});

describe('drop on a group label', () => {
	it('shows the label’s pill and adds the tab to the group', async () => {
		const h = await setup();
		const group = await h.api.createGroup([h.ids.a!, h.ids.b!]);
		await h.refresh();
		h.press({ tab: h.ids.d! }, { x: 450 });
		h.move(30, 20);
		expect(h.state().target).toEqual({ outcome: 'overGroupLabel', group });
		expect(h.state().pill).toMatchObject({
			kind: 'add',
			text: 'Release to add to Group 1',
			announce: 'Drag: release to add to group 1'.replace('group 1', 'Group 1'),
		});
		h.up(30, 20);
		await h.settle();
		expect(h.snapshot().tabs.map((tab) => tab.group)).toEqual([group, group, group, null, null]);
		expect(order(h.snapshot())).toEqual(['a', 'b', 'd', 'c', 'e']);
		expect(h.live).toContain('Added d to Group 1, now 3 tabs');
	});

	it('takes a pair in together and counts both panes', async () => {
		const h = await setup();
		const group = await h.api.createGroup([h.ids.a!, h.ids.b!]);
		await h.api.joinPair([h.ids.d!, h.ids.e!], 'sideBySide');
		await h.refresh();
		h.press({ tab: h.ids.d! }, { x: 450 });
		h.move(30, 20);
		h.up(30, 20);
		await h.settle();
		const grouped = h.snapshot().tabs.filter((tab) => tab.group === group);
		expect(grouped.map((tab) => tab.id)).toEqual([h.ids.a, h.ids.b, h.ids.d, h.ids.e]);
		expect(h.live).toContain('Added d to Group 1, now 4 tabs');
	});

	it('does nothing over the chip of the group the tab is already in', async () => {
		const h = await setup();
		await h.api.createGroup([h.ids.a!, h.ids.b!]);
		await h.refresh();
		h.press({ tab: h.ids.b! }, { x: 200 });
		h.move(30, 20);
		expect(h.state().target?.outcome).not.toBe('overGroupLabel');
	});
});

describe('leave a group', () => {
	it('shows the group’s name, and the end state matches the preview', async () => {
		const h = await setup();
		const group = await h.api.createGroup([h.ids.a!, h.ids.b!]);
		await h.refresh();
		// Strip: [chip 0-60] a 60-160, b 160-260, c 260-360, d, e. Drag a past the group's end.
		h.press({ tab: h.ids.a! }, { x: 110 });
		h.move(340);
		const target = h.state().target as Extract<TabDragTarget, { outcome: 'leaveGroup' }>;
		expect(target).toMatchObject({ outcome: 'leaveGroup', group, to: 2 });
		expect(h.state().pill).toMatchObject({ kind: 'leave', text: 'Release to leave Group 1' });
		h.up(340);
		await h.settle();
		const snapshot = h.snapshot();
		// Two commands (Remove from Group, then Move) land where the preview said.
		expect(snapshot.tabs.findIndex((tab) => tab.id === h.ids.a)).toBe(target.to);
		expect(order(snapshot)).toEqual(['b', 'c', 'a', 'd', 'e']);
		expect(snapshot.tabs.find((tab) => tab.id === h.ids.a)?.group).toBeNull();
		expect(snapshot.tabs.find((tab) => tab.id === h.ids.b)?.group).toBe(group);
		expect(h.live).toContain('Removed a from Group 1');
	});

	it('leaves to the left of the group as well', async () => {
		const h = await setup();
		const group = await h.api.createGroup([h.ids.b!, h.ids.c!]);
		await h.refresh();
		// Strip: a 0-100 [chip 100-160] b 160-260, c 260-360. Drag c left of the chip.
		h.press({ tab: h.ids.c! }, { x: 310 });
		h.move(40);
		expect(h.state().target).toMatchObject({ outcome: 'leaveGroup', group, to: 0 });
		h.up(40);
		await h.settle();
		expect(order(h.snapshot())).toEqual(['c', 'a', 'b', 'd', 'e']);
		expect(h.snapshot().tabs[0]?.group).toBeNull();
	});

	it('stays inside the group while the tab is within its span', async () => {
		const h = await setup();
		const group = await h.api.createGroup([h.ids.a!, h.ids.b!]);
		await h.refresh();
		h.press({ tab: h.ids.a! }, { x: 110 });
		h.move(260);
		expect(h.state().target).toMatchObject({ outcome: 'reorder', to: 1 });
		h.up(260);
		await h.settle();
		expect(order(h.snapshot())).toEqual(['b', 'a', 'c', 'd', 'e']);
		expect(h.snapshot().tabs[0]?.group).toBe(group);
	});
});

describe('split a pane', () => {
	// `e` is the active tab, the pane on show; the area is x 0-1000 and y 200-600.
	it.each([
		['left', 100, 400, 'sideBySide', 'a', 'e'],
		['right', 900, 400, 'sideBySide', 'e', 'a'],
		['top', 500, 250, 'stacked', 'a', 'e'],
		['bottom', 500, 550, 'stacked', 'e', 'a'],
	] as const)('splits %s with the view on show', async (edge, x, y, layoutName, first, second) => {
		const h = await setup();
		h.press({ tab: h.ids.a! }, { x: 50 });
		h.move(x, y);
		expect(h.state().target).toEqual({ outcome: 'splitPane', edge });
		expect(h.state().pill?.text).toBe(`Split ${edge} with the current view`);
		h.up(x, y);
		await h.settle();
		const pair = h.snapshot().pairs[0]!;
		expect(pair.layout).toBe(layoutName);
		expect(pair.panes).toEqual([h.ids[first], h.ids[second]]);
		expect(h.live).toContain(`Split ${first} and ${second}`);
	});

	it.each([
		['left', 100, 400, 'sideBySide', true],
		['right', 900, 400, 'sideBySide', false],
		['top', 500, 250, 'stacked', true],
		['bottom', 500, 550, 'stacked', false],
	] as const)(
		'splits the active tab itself %s, as F3 does, with the copy on that side',
		async (edge, x, y, layoutName, copyFirst) => {
			const h = await setup();
			h.press({ tab: h.ids.e! }, { x: 450 });
			h.move(x, y);
			expect(h.state().target).toEqual({ outcome: 'splitPane', edge });
			h.up(x, y);
			await h.settle();
			const pair = h.snapshot().pairs[0]!;
			expect(pair.layout).toBe(layoutName);
			expect(pair.origin.kind).toBe('toggle');
			const created = pair.origin.kind === 'toggle' ? pair.origin.created : -1;
			expect(created).not.toBe(h.ids.e);
			expect(pair.panes).toEqual(copyFirst ? [created, h.ids.e] : [h.ids.e, created]);
			expect(h.snapshot().active).toBe(created);
		},
	);

	it('covers the whole area: the regions meet at the thirds and at the centre column halves', async () => {
		const h = await setup();
		h.press({ tab: h.ids.a! }, { x: 50 });
		const edgeAt = (x: number, y: number) => {
			h.move(x, y);
			const target = h.state().target;
			return target?.outcome === 'splitPane' ? target.edge : target?.outcome;
		};
		expect(edgeAt(1000 / 3 - 20, 400)).toBe('left');
		expect(edgeAt(1000 / 3 + 20, 300)).toBe('top');
		expect(edgeAt(1000 / 3 + 20, 500)).toBe('bottom');
		expect(edgeAt(1000 - 1000 / 3 + 20, 400)).toBe('right');
		expect(edgeAt(500, 392)).toBe('top');
		expect(edgeAt(500, 408)).toBe('bottom');
	});

	it('is offered only over the file area, so the strip, the toolbar and the sidebar are the strip’s', async () => {
		const h = await setup();
		h.press({ tab: h.ids.a! }, { x: 50 });
		// The toolbar, between the strip and the area.
		h.move(500, 120);
		expect(h.state().target?.outcome).toBe('reorder');
		// The status bar, below it.
		h.move(500, 650);
		expect(h.state().target?.outcome).toBe('reorder');
		expect(h.state().pill?.kind).toBe('move');
	});

	it('is not offered over a pair, for a pair, or when the tabs cannot join', async () => {
		const h = await setup();
		await h.api.pinTab(h.ids.a!, true);
		await h.refresh();
		// A pinned tab cannot join the unpinned view on show.
		h.press({ tab: h.ids.a! }, { x: 50 });
		h.move(100, 400);
		expect(h.state().target?.outcome).toBe('reorder');
		h.session.cancel();

		await h.api.joinPair([h.ids.d!, h.ids.e!], 'sideBySide');
		await h.refresh();
		h.press({ tab: h.ids.b! }, { x: 150 });
		h.move(100, 400);
		expect(h.state().target?.outcome).toBe('reorder');
		h.session.cancel();
		h.press({ tab: h.ids.d! }, { x: 350 });
		h.move(100, 400);
		expect(h.state().target?.outcome).not.toBe('splitPane');
	});

	it('cancels with Escape, and when the pointer goes back out of the area', async () => {
		const h = await setup();
		const joinPair = vi.spyOn(h.api, 'joinPair');
		h.press({ tab: h.ids.a! }, { x: 50 });
		h.move(100, 400);
		expect(h.state().target?.outcome).toBe('splitPane');
		fireEvent.keyDown(document.body, { key: 'Escape' });
		expect(h.state().phase).toBe('cancelled');
		h.up(100, 400);
		await h.settle();
		expect(joinPair).not.toHaveBeenCalled();
		expect(h.live[h.live.length - 1]).toBe('Drag cancelled');

		h.press({ tab: h.ids.a! }, { x: 50 });
		h.move(100, 400);
		h.move(100, 100);
		expect(h.state().target?.outcome).toBe('reorder');
		h.up(100, 100);
		await h.settle();
		expect(joinPair).not.toHaveBeenCalled();
	});

	it('does not flicker at the area’s edge or between regions', async () => {
		const h = await setup();
		h.press({ tab: h.ids.a! }, { x: 50 });
		h.move(100, 400);
		// A few pixels above the area, or across a border, keeps the region it was in.
		h.move(100, 197);
		expect(h.state().target).toEqual({ outcome: 'splitPane', edge: 'left' });
		h.move(1000 / 3 + 3, 400);
		expect(h.state().target).toEqual({ outcome: 'splitPane', edge: 'left' });
		h.move(1000 / 3 + 12, 400);
		expect(h.state().target).toEqual({ outcome: 'splitPane', edge: 'top' });
		// Well out of it is the strip's again.
		h.move(100, 150);
		expect(h.state().target?.outcome).toBe('reorder');
		// Coming in from outside has no slack.
		h.move(100, 197);
		expect(h.state().target?.outcome).toBe('reorder');
	});
});

describe('the new-window phase', () => {
	it('starts only when the pointer leaves the window, shows no pill and a release does nothing yet', async () => {
		const h = await setup();
		const spies = [
			'moveTab',
			'joinPair',
			'createGroup',
			'addToGroup',
			'removeFromGroup',
			'moveTabs',
		].map((name) => vi.spyOn(h.api, name as 'moveTab'));
		h.press({ tab: h.ids.a! }, { x: 50 });
		// Far below the strip, over the toolbar, the area and the status bar, it is still the window's.
		for (const y of [60, 150, 400, VIEW.height - 1]) {
			h.move(60, y);
			expect(h.state().target?.outcome).not.toBe('newWindow');
		}
		h.move(60, 0);
		expect(h.state().target?.outcome).not.toBe('newWindow');
		h.move(60, -1);
		expect(h.state().target).toEqual({ outcome: 'newWindow' });
		expect(h.state().pill).toBeNull();
		h.move(VIEW.width, 100);
		expect(h.state().target).toEqual({ outcome: 'newWindow' });
		h.move(VIEW.width - 1, 100);
		expect(h.state().target?.outcome).not.toBe('newWindow');
		h.move(60, VIEW.height);
		expect(h.state().target).toEqual({ outcome: 'newWindow' });
		h.up(60, VIEW.height + 40);
		await h.settle();
		for (const spy of spies) expect(spy).not.toHaveBeenCalled();
		expect(order(h.snapshot())).toEqual(['a', 'b', 'c', 'd', 'e']);
	});

	it('is also entered from the file area, and from a split region', async () => {
		const h = await setup();
		h.press({ tab: h.ids.a! }, { x: 50 });
		h.move(100, 400);
		expect(h.state().target?.outcome).toBe('splitPane');
		h.move(-30, 400);
		expect(h.state().target).toEqual({ outcome: 'newWindow' });
	});

	it('starts when the document says the pointer left, at its last position', async () => {
		const update = vi.fn(() => ({ kind: 'window', text: 'Release to open in a new window' }));
		const h = await setup(undefined, { update });
		h.press({ tab: h.ids.a! }, { x: 50 });
		h.move(100, 400);
		expect(update).not.toHaveBeenCalled();
		h.leftWindow();
		expect(h.state().target).toEqual({ outcome: 'newWindow' });
		expect(update).toHaveBeenCalledWith({ x: 100, y: 400 }, expect.anything());
		// The next move inside the window is a plain move again.
		h.move(110, 400);
		expect(h.state().target?.outcome).toBe('splitPane');
	});

	it('hands the hook the pointer, a pill, the way back and the release', async () => {
		const update = vi.fn(() => ({ kind: 'window', text: 'Release to open in a new window' }));
		const leave = vi.fn();
		const drop = vi.fn((_point: { x: number; y: number }) => true);
		const h = await setup(undefined, { update, leave, drop });
		h.press({ tab: h.ids.a! }, { x: 50 });
		h.move(60, 100);
		expect(update).not.toHaveBeenCalled();
		h.move(60, -40);
		expect(update).toHaveBeenCalled();
		expect(h.state().pill?.text).toBe('Release to open in a new window');
		// Back in the window it is a reorder again, and the hook is told.
		h.move(60, 15);
		expect(leave).toHaveBeenCalledTimes(1);
		expect(h.state().target?.outcome).toBe('reorder');
		expect(h.state().pill?.kind).toBe('move');
		h.move(60, -40);
		h.up(60, -40);
		await h.settle();
		expect(drop).toHaveBeenCalledTimes(1);
		expect(drop.mock.calls[0]![0]).toEqual({ x: 60, y: -40 });
	});
});

describe('cancel', () => {
	it('is Escape, which changes nothing and says so', async () => {
		const h = await setup();
		const moveTab = vi.spyOn(h.api, 'moveTab');
		h.press({ tab: h.ids.a! }, { x: 50 });
		h.move(262);
		fireEvent.keyDown(document.body, { key: 'Escape' });
		expect(h.state().phase).toBe('cancelled');
		expect(h.state().pill).toBeNull();
		h.up(262);
		await h.settle();
		expect(moveTab).not.toHaveBeenCalled();
		expect(h.live[h.live.length - 1]).toBe('Drag cancelled');
		expect(h.session.consumeClick()).toBe(true);
	});

	it('also ends the holds, so nothing arms after it', async () => {
		const h = await setup();
		h.press({ tab: h.ids.a! }, { x: 50 });
		h.move(150);
		fireEvent.keyDown(document.body, { key: 'Escape' });
		h.clock.advance(5000);
		// Had the hold survived it would have armed and written a target into the idle store.
		expect(h.state()).toMatchObject({ phase: 'idle', target: null, pill: null });
	});
});

describe('the pill', () => {
	it('is one at a time, and each outcome says its own thing', async () => {
		const h = await setup();
		const group = await h.api.createGroup([h.ids.a!, h.ids.b!]);
		await h.refresh();
		const texts: string[] = [];
		h.press({ tab: h.ids.d! }, { x: 450 });
		for (const [x, y] of [
			[400, 15],
			[30, 20],
			[500, -20],
			[40, 400],
			[400, 15],
		] as const) {
			h.move(x, y);
			texts.push(h.state().pill?.text ?? '(none)');
		}
		expect(texts).toEqual([
			'Release to move d to position 4 of 5',
			`Release to add to Group ${group}`,
			'(none)',
			'Split left with the current view',
			'Release to move d to position 4 of 5',
		]);
	});
});
