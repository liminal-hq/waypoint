// Tests for the command bridge and the feed that keeps its facts current from the panes, the queue and the view
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createViewStore } from '../browse/viewStore';
import { createSidebarStore } from '../sidebar/sidebarStore';
import { commandsHarness, select, type CommandsHarness } from '../test/fileCommandsHarness';
import { CommandBridgeProvider, createCommandBridge, useCommands } from './commandBridge';
import { startCommandFeed } from './commandFeed';
import { emptyFacts, idleActions } from './commandEnv';

afterEach(cleanup);

async function setup() {
	const h: CommandsHarness = await commandsHarness();
	const bridge = createCommandBridge();
	const view = createViewStore();
	const sidebar = createSidebarStore();
	let active: typeof h.session | null = h.session;
	const panes = new Set<() => void>();
	const feed = startCommandFeed(bridge, {
		files: h.commands,
		activeSession: () => active,
		ops: h.ops,
		clipboard: null,
		view,
		sidebar,
		subscribePanes: (listener) => {
			panes.add(listener);
			return () => panes.delete(listener);
		},
	});
	return {
		h,
		bridge,
		view,
		sidebar,
		feed,
		facts: () => bridge.store.getState().facts,
		close: () => {
			active = null;
			for (const listener of panes) listener();
		},
	};
}

describe('the bridge', () => {
	it('starts with a window that has nothing open', () => {
		const bridge = createCommandBridge();
		expect(bridge.store.getState().facts).toEqual(emptyFacts());
	});

	it('does not notify when a patch changes nothing', () => {
		const bridge = createCommandBridge();
		const listener = vi.fn();
		bridge.store.subscribe(listener);
		bridge.patchFacts({ showHidden: false, sort: null, history: [] });
		expect(listener).not.toHaveBeenCalled();
		bridge.patchFacts({ showHidden: true });
		expect(listener).toHaveBeenCalledTimes(1);
	});

	it('hands `useCommands` the registry resolved for the facts, and a stable `run`', () => {
		const bridge = createCommandBridge();
		const newTab = vi.fn();
		bridge.patchActions({ newTab });
		const { result } = renderHook(() => useCommands(), {
			wrapper: ({ children }) => (
				<CommandBridgeProvider value={bridge}>{children}</CommandBridgeProvider>
			),
		});
		const run = result.current.run;
		expect(result.current.get('newTab').enabled).toBe(true);
		expect(result.current.get('rename').visible).toBe(false);
		act(() => {
			bridge.patchFacts({ tab: true, listing: true });
		});
		expect(result.current.facts.tab).toBe(true);
		expect(result.current.run).toBe(run);
		expect(result.current.run('newTab')).toBe(true);
		expect(newTab).toHaveBeenCalled();
		expect(result.current.run('closeTab')).toBe(true);
	});

	it('lists nothing runnable without a provider', () => {
		const { result } = renderHook(() => useCommands());
		expect(result.current.run('newFolder')).toBe(false);
		expect(idleActions().files).toBeNull();
	});
});

describe('the feed', () => {
	it('reports the open pane: its sort, whether it can be written to, and nothing selected', async () => {
		const s = await setup();
		expect(s.facts()).toMatchObject({
			listing: true,
			trash: false,
			selected: 0,
			batchRename: false,
		});
		expect(s.facts().sort).toMatchObject({ key: 'name' });
		expect(s.facts().file.newFolder).toEqual({ visible: true, enabled: true });
		expect(s.facts().file.copy).toEqual({ visible: true, enabled: false });
	});

	it('follows the selection and enables what acts on it', async () => {
		const s = await setup();
		await select(s.h, 0, 1);
		expect(s.facts().selected).toBe(2);
		expect(s.facts().batchRename).toBe(true);
		expect(s.facts().file.copy).toEqual({ visible: true, enabled: true });
		s.h.session.store.getState().deselectAll();
		expect(s.facts().selected).toBe(0);
		expect(s.facts().file.copy.enabled).toBe(false);
	});

	it('follows the view and the sidebar', async () => {
		const s = await setup();
		s.view.getState().setMode('grid');
		s.view.getState().toggleHidden();
		s.sidebar.getState().toggleOpen();
		expect(s.facts()).toMatchObject({ viewMode: 'grid', showHidden: true, sidebarOpen: false });
	});

	it('follows the history: the labels and the two heads, then the entries newest first', async () => {
		const s = await setup();
		s.h.commands.newFolder();
		const id = await s.h.finish(1, 'New folder');
		await waitFor(() => expect(s.facts().undoLabel).toBe('New folder'));
		expect(s.facts().undoHead).not.toBeNull();
		expect(s.facts().file.undo.enabled).toBe(true);
		await waitFor(() =>
			expect(s.facts().history.map((entry) => entry.label)).toEqual(['New folder']),
		);
		expect(id).toBeGreaterThan(0);
	});

	it('shows nothing selected and no listing once the pane closes', async () => {
		const s = await setup();
		await select(s.h, 0);
		s.close();
		expect(s.facts()).toMatchObject({ listing: false, selected: 0, sort: null });
	});

	it('stops following when it is stopped', async () => {
		const s = await setup();
		s.feed.stop();
		s.view.getState().setMode('grid');
		expect(s.facts().viewMode).toBe('list');
	});
});
