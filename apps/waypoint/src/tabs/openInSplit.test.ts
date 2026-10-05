// Verifies opening a folder in a new pane: the layout and order per edge, focus, and the refusals
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { fileLocation } from '../services/fakeVfsClient';
import { canOpenInSplit, openInSplit } from './openInSplit';

const HOME = fileLocation('/home/test');
const DOCS = fileLocation('/home/test/docs');

async function setup() {
	const api = new FakeTabsApi();
	await api.openTab(HOME);
	const announced: string[] = [];
	// The page keeps the newest snapshot; the test refreshes it where the session would push one.
	let current = await api.getSnapshot();
	const deps = {
		api,
		snapshot: () => current,
		announce: (text: string) => void announced.push(text),
	};
	const refresh = async () => (current = await api.getSnapshot());
	return { api, deps, announced, refresh };
}

describe('opening a folder in a split pane', () => {
	it.each([
		['right', 'sideBySide', false],
		['left', 'sideBySide', true],
		['bottom', 'stacked', false],
		['top', 'stacked', true],
	] as const)('on the %s: %s, new pane first is %s', async (edge, layout, first) => {
		const { api, deps, announced } = await setup();
		const before = (await api.getSnapshot()).active!;
		expect(await openInSplit(deps, DOCS, edge)).toBe(true);
		const snapshot = await api.getSnapshot();
		const pair = snapshot.pairs[0]!;
		const created = snapshot.tabs.find((tab) => tab.location.uri === DOCS.uri)!;
		expect(pair.layout).toBe(layout);
		expect(pair.panes[first ? 0 : 1]).toBe(created.id);
		expect(pair.panes[first ? 1 : 0]).toBe(before);
		expect(snapshot.active).toBe(created.id);
		expect(announced).toEqual(['Opened docs in a new pane']);
	});

	it('is refused, with the reason said, when the tab is already in a pair', async () => {
		const { api, deps, announced, refresh } = await setup();
		await openInSplit(deps, DOCS);
		await refresh();
		expect(canOpenInSplit(await api.getSnapshot())).toBe(false);
		const count = (await api.getSnapshot()).tabs.length;
		expect(await openInSplit(deps, DOCS)).toBe(false);
		expect((await api.getSnapshot()).tabs).toHaveLength(count);
		expect(announced.at(-1)).toBe('Cannot open in a new pane: this tab is already split');
	});
});
