// Runs the shared conformance scenarios of the Rust store against FakeTabsApi
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The scenarios live in `crates/waypoint-session/tests/conformance/*.json` and the format is
// documented in `tests/conformance.rs`. Both runners must pass every step, so a behaviour change
// in the reducer shows up here until the fake follows.

import { describe, expect, it } from 'vitest';
import basic from '../../../../crates/waypoint-session/tests/conformance/basic.json';
import groupsAndPairs from '../../../../crates/waypoint-session/tests/conformance/groups_and_pairs.json';
import windowsAndHandoff from '../../../../crates/waypoint-session/tests/conformance/windows_and_handoff.json';
import workspacesScenario from '../../../../crates/waypoint-session/tests/conformance/workspaces.json';
import type { GroupSort } from '@liminal-hq/waypoint-protocol/generated/GroupSort';
import type { MoveTo } from '@liminal-hq/waypoint-protocol/generated/MoveTo';
import type { MoveWhat } from '@liminal-hq/waypoint-protocol/generated/MoveWhat';
import type { PairLayout } from '@liminal-hq/waypoint-protocol/generated/PairLayout';
import type { SessionEvent } from '@liminal-hq/waypoint-protocol/generated/SessionEvent';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import type { TabColour } from '@liminal-hq/waypoint-protocol/generated/TabColour';
import type { WindowState } from '@liminal-hq/waypoint-protocol/generated/WindowState';
import { FakeTabsApi } from './fakeTabsApi';
import { FakeTabsStore } from './fakeTabsStore';
import { fileLocation } from './fakeVfsClient';
import { applyTabsEvent } from './tabsApi';

interface WindowFacts {
	tabs?: number[];
	active?: number | null;
	locations?: string[];
	pinned?: number[];
	history?: Record<string, { back: string[]; forward: string[] }>;
	mru?: number[];
	groups?: { id: number; name: string; collapsed: boolean; tabs: number[] }[];
	pairs?: { id: number; panes: number[]; layout: string; sizes: number[] }[];
	workspace?: number | null;
}

interface Facts extends WindowFacts {
	closed?: number[];
	windows?: string[];
	workspaces?: { id: number; name: string; locations: string[] }[];
	others?: Record<string, WindowFacts>;
}

type Op = Record<string, unknown> & { op: string };

interface Step {
	window?: string;
	do: Op;
	/** Omitted by a step that only checks it fails. */
	expect?: Facts;
	error?: boolean;
	note?: string;
}

interface Scenario {
	description: string;
	steps: Step[];
}

const scenarios: Record<string, Scenario> = {
	basic,
	groups_and_pairs: groupsAndPairs,
	windows_and_handoff: windowsAndHandoff,
	workspaces: workspacesScenario,
} as Record<string, Scenario>;

const path = (op: Op, key: string) => fileLocation(String(op[key]));
const num = (op: Op, key: string) => Number(op[key]);
const list = (op: Op, key: string) => (op[key] as number[]).map(Number);

/** Runs one scenario operation against the fake, the way `command()` in the Rust runner does. */
async function perform(api: FakeTabsApi, op: Op): Promise<void> {
	switch (op.op) {
		case 'open':
			await api.openTab(path(op, 'location'), {
				after: op.after === undefined ? undefined : num(op, 'after'),
				activate: typeof op.activate === 'boolean' ? op.activate : true,
			});
			return;
		case 'close':
			return api.closeTab(num(op, 'tab'));
		case 'activate':
			return api.activateTab(num(op, 'tab'));
		case 'back':
			return api.back(num(op, 'tab'));
		case 'forward':
			return api.forward(num(op, 'tab'));
		case 'move':
			return api.moveTab(num(op, 'tab'), num(op, 'index'));
		case 'navigate':
			return api.navigate(num(op, 'tab'), path(op, 'location'));
		case 'pin':
			return api.pinTab(num(op, 'tab'), op.pinned === true);
		case 'colour':
			return api.setTabColour(num(op, 'tab'), op.colour as TabColour | null);
		case 'reopen':
			await api.reopenTab(op.tab === undefined ? undefined : num(op, 'tab'));
			return;
		case 'createGroup':
			await api.createGroup(list(op, 'tabs'), op.name === undefined ? undefined : String(op.name));
			return;
		case 'addToGroup':
			return api.addToGroup(num(op, 'tab'), num(op, 'group'));
		case 'removeFromGroup':
			return api.removeFromGroup(num(op, 'tab'));
		case 'renameGroup':
			return api.renameGroup(num(op, 'group'), String(op.name));
		case 'collapseGroup':
			return api.setGroupCollapsed(num(op, 'group'), op.collapsed === true);
		case 'collapseOthers':
			return api.collapseOtherGroups(num(op, 'group'));
		case 'sortGroup':
			return api.sortGroup(num(op, 'group'), op.by as GroupSort);
		case 'moveGroup':
			return api.moveGroup(num(op, 'group'), num(op, 'index'));
		case 'duplicateGroup':
			await api.duplicateGroup(num(op, 'group'));
			return;
		case 'ungroup':
			return api.ungroup(num(op, 'group'));
		case 'closeGroup':
			return api.closeGroup(num(op, 'group'));
		case 'joinPair':
			await api.joinPair(list(op, 'tabs'), (op.layout ?? 'sideBySide') as PairLayout);
			return;
		case 'separatePair':
			return api.separatePair(num(op, 'pair'));
		case 'swapPanes':
			return api.swapPanes(num(op, 'pair'));
		case 'setPairSizes':
			return api.setPairSizes(num(op, 'pair'), list(op, 'sizes'));
		case 'toggleSplit':
			return api.toggleSplit(num(op, 'tab'));
		case 'saveGroupAsWorkspace':
			await api.saveGroupAsWorkspace(
				num(op, 'group'),
				op.name === undefined ? undefined : String(op.name),
			);
			return;
		case 'renameWorkspace':
			return api.renameWorkspace(num(op, 'workspace'), String(op.name));
		case 'deleteWorkspace':
			return api.deleteWorkspace(num(op, 'workspace'));
		case 'setActiveWorkspace':
			return api.setActiveWorkspace(op.workspace === null ? null : num(op, 'workspace'));
		case 'setWorkspaceLocations':
			return api.setWorkspaceLocations(
				num(op, 'workspace'),
				(op.locations as string[]).map((l) => fileLocation(l)),
			);
		case 'openWindow':
			await api.openWindow(op.location === undefined ? undefined : path(op, 'location'));
			return;
		case 'closeWindow':
			return api.closeWindow();
		case 'moveTabs': {
			// A new window in the scenario format has no label or geometry.
			const to = op.to as { kind: string; label?: string | null };
			const wire: MoveTo =
				to.kind === 'newWindow'
					? { kind: 'newWindow', label: null, geometry: null }
					: (op.to as MoveTo);
			await api.moveTabs(op.what as MoveWhat, wire);
			return;
		}
		default:
			throw new Error(`unknown op ${op.op}`);
	}
}

const WINDOW_FACTS = [
	'tabs',
	'active',
	'locations',
	'pinned',
	'history',
	'mru',
	'groups',
	'pairs',
	'workspace',
];

/** The window facts of a snapshot, as the scenarios state them. */
function checkWindow(w: WindowState | SessionSnapshot, want: WindowFacts, at: string) {
	if (want.tabs)
		expect(
			w.tabs.map((t) => t.id),
			`${at}: tabs`,
		).toEqual(want.tabs);
	if (want.active !== undefined) expect(w.active, `${at}: active`).toBe(want.active);
	if (want.locations) {
		expect(
			w.tabs.map((t) => t.location.display),
			`${at}: locations`,
		).toEqual(want.locations);
	}
	if (want.pinned) {
		expect(
			w.tabs.filter((t) => t.pinned).map((t) => t.id),
			`${at}: pinned`,
		).toEqual(want.pinned);
	}
	for (const [tab, history] of Object.entries(want.history ?? {})) {
		const t = w.tabs.find((x) => x.id === Number(tab));
		expect(t, `${at}: tab ${tab}`).toBeDefined();
		expect(
			t?.back.map((l) => l.display),
			`${at}: back of ${tab}`,
		).toEqual(history.back);
		expect(
			t?.forward.map((l) => l.display),
			`${at}: forward of ${tab}`,
		).toEqual(history.forward);
	}
	if (want.mru) expect(w.mru, `${at}: mru`).toEqual(want.mru);
	if (want.groups) {
		const got = w.groups.map((g) => ({
			id: g.id,
			name: g.name,
			collapsed: g.collapsed,
			tabs: w.tabs.filter((t) => t.group === g.id).map((t) => t.id),
		}));
		expect(got, `${at}: groups`).toEqual(want.groups);
	}
	if (want.pairs) {
		const got = w.pairs.map((p) => ({
			id: p.id,
			panes: p.panes,
			layout: p.layout,
			sizes: p.sizes,
		}));
		expect(got, `${at}: pairs`).toEqual(want.pairs);
	}
	if (want.workspace !== undefined) expect(w.workspace, `${at}: workspace`).toBe(want.workspace);
}

/** One open window: its handle, what it heard, and the snapshot rebuilt from those events. */
interface Attached {
	api: FakeTabsApi;
	events: SessionEvent[];
	mirror: SessionSnapshot;
}

describe.each(Object.entries(scenarios))('conformance scenario %s', (name, scenario) => {
	it("passes every step on the fake, and every window's events rebuild its snapshot", async () => {
		// The Rust runner starts from `Store::new()` with `main-1` registered, which is what a
		// shared fake store does when its first window attaches.
		const store = new FakeTabsStore();
		const attached = new Map<string, Attached>();
		const attach = async (label: string) => {
			const api = new FakeTabsApi(store, label);
			const events: SessionEvent[] = [];
			api.onEvent((e) => events.push(e));
			attached.set(label, { api, events, mirror: await api.getSnapshot() });
		};
		await attach('main-1');

		for (const [n, step] of scenario.steps.entries()) {
			const at = `${name} step ${n + 1}`;
			const want = step.expect ?? {};
			const label = step.window ?? 'main-1';
			const handle = attached.get(label);
			if (!handle) throw new Error(`${at}: ${label} is not open`);
			const before = store.toSnapshot();
			if (step.error) {
				await expect(perform(handle.api, step.do), `${at}: expected an error`).rejects.toBeTypeOf(
					'string',
				);
				expect(store.toSnapshot(), `${at}: an error changes nothing`).toEqual(before);
			} else {
				await perform(handle.api, step.do);
			}
			expect(store.violations(), `${at}: invariants`).toEqual([]);

			// Windows the step made get a handle, like a webview that starts and reads its snapshot.
			for (const open of store.windowLabels()) if (!attached.has(open)) await attach(open);
			for (const gone of [...attached.keys()])
				if (!store.windowLabels().includes(gone)) attached.delete(gone);

			if (WINDOW_FACTS.some((k) => k in want)) {
				checkWindow(await handle.api.getSnapshot(), want, at);
			}
			if (want.closed) {
				expect(
					store.closed().map((c) => c.tab.id),
					`${at}: closed`,
				).toEqual(want.closed);
			}
			if (want.workspaces) {
				expect(
					store.toSnapshot().workspaces.map((w) => ({
						id: w.id,
						name: w.name,
						locations: w.locations.map((l) => l.display),
					})),
					`${at}: workspaces`,
				).toEqual(want.workspaces);
			}
			if (want.windows) expect(store.windowLabels(), `${at}: windows`).toEqual(want.windows);
			for (const [other, facts] of Object.entries(want.others ?? {})) {
				const w = store.window(other);
				expect(w, `${at}: ${other} is open`).toBeDefined();
				if (w) checkWindow(w, facts, `${at} (${other})`);
			}

			// What a window heard rebuilds what it holds (the closed list has no event).
			for (const a of attached.values()) {
				for (const event of a.events.splice(0)) a.mirror = applyTabsEvent(a.mirror, event);
				const snapshot = await a.api.getSnapshot();
				// A window hears only its own events, so its revision can trail the global one.
				expect(a.mirror.revision).toBeLessThanOrEqual(snapshot.revision);
				expect(
					{ ...a.mirror, revision: 0, closed: [] },
					`${at}: ${a.api.label} events rebuild it`,
				).toEqual({ ...snapshot, revision: 0, closed: [] });
			}
		}
	});
});
