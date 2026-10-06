// Verifies each shortcut the registry displays is the key the window's existing hooks actually bind
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, renderHook } from '@testing-library/react';
import type { KeyboardEvent as ReactKeyboardEvent } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { useListInteractions } from '../browse/useListInteractions';
import { createViewStore } from '../browse/viewStore';
import { startRefreshShortcut } from '../browse/refreshRules';
import { useViewShortcuts } from '../browse/useViewShortcuts';
import { isBatchRenameKey } from '../ops/batchRename/useBatchRenameShortcut';
import { handleFileKey, isRestoreKey, type FileKeyHandlers } from '../ops/useFileShortcuts';
import { helpPageForKey } from '../help/helpModel';
import { isSettingsShortcut } from '../settings/openSettingsWindow';
import { createSidebarStore } from '../sidebar/sidebarStore';
import { createInspectorStore } from '../inspector/inspectorStore';
import { useInspectorShortcuts } from '../inspector/useInspectorShortcuts';
import { isPropertiesWindowKey } from '../inspector/usePropertiesWindowShortcut';
import { createShelfStore } from '../shelf/shelfStore';
import { useShelfShortcuts } from '../shelf/useShelfShortcuts';
import { useSidebarShortcuts } from '../sidebar/useSidebarShortcuts';
import { handlePairKey } from '../tabs/usePairShortcuts';
import { handleTabKey, type TabKeyHandlers } from '../tabs/useTabShortcuts';
import { handleWindowKey } from '../tabs/windowActions';
import { commandsHarness, select } from '../test/fileCommandsHarness';
import { isPaletteKey } from './CommandPaletteHost';
import { COMMANDS, type CommandId } from './registry';
import { keyEventInit, parseShortcut } from './shortcuts';

const press = (shortcut: string) => keyEventInit(shortcut) as KeyboardEventInit & { key: string };

const fileHandlers = () =>
	({
		newFolder: vi.fn(),
		newFile: vi.fn(),
		rename: vi.fn(),
		duplicate: vi.fn(),
		moveToTrash: vi.fn(),
		deletePermanently: vi.fn(),
		cut: vi.fn(),
		copy: vi.fn(),
		paste: vi.fn(),
		copyToOtherPane: vi.fn(),
		moveToOtherPane: vi.fn(),
		undo: vi.fn(),
		redo: vi.fn(),
	}) satisfies FileKeyHandlers;

/** Which handler of `handleFileKey` a shortcut calls, and nothing else. */
function fileKeyCalls(shortcut: string): string[] {
	const handlers = fileHandlers();
	const init = press(shortcut);
	const took = handleFileKey(
		{
			key: init.key,
			ctrlKey: !!init.ctrlKey,
			metaKey: false,
			altKey: !!init.altKey,
			shiftKey: !!init.shiftKey,
			isComposing: false,
		},
		handlers,
	);
	const called = Object.entries(handlers)
		.filter(([, spy]) => spy.mock.calls.length > 0)
		.map(([name]) => name);
	return took ? called : [];
}

function tabKeyCalls(shortcut: string): string[] {
	const calls: string[] = [];
	const handlers: TabKeyHandlers = {
		newTab: () => calls.push('newTab'),
		close: () => calls.push('close'),
		goTo: () => calls.push('goTo'),
		reopenClosed: () => calls.push('reopenClosed'),
		switchStep: () => calls.push('switchStep'),
	};
	const init = press(shortcut);
	handleTabKey(
		{
			key: init.key,
			ctrlKey: !!init.ctrlKey,
			metaKey: false,
			altKey: !!init.altKey,
			shiftKey: !!init.shiftKey,
		},
		handlers,
		1,
	);
	return calls;
}

function dispatchOnWindow(shortcut: string) {
	act(() => {
		fireEvent.keyDown(window, keyEventInit(shortcut));
	});
}

/** A synthetic list key event aimed at the list itself, which is what the list's own handler accepts. */
function listKey(shortcut: string): ReactKeyboardEvent<HTMLElement> {
	const target = document.createElement('div');
	const init = press(shortcut);
	return {
		key: init.key,
		ctrlKey: !!init.ctrlKey,
		metaKey: false,
		altKey: !!init.altKey,
		shiftKey: !!init.shiftKey,
		nativeEvent: { isComposing: false },
		target,
		currentTarget: target,
		preventDefault: () => {},
	} as unknown as ReactKeyboardEvent<HTMLElement>;
}

/** What binds each command's key. A command with a shortcut and no probe here fails the coverage test below. */
const probes: Partial<Record<CommandId, (shortcut: string) => void | Promise<void>>> = {
	newFolder: (s) => expect(fileKeyCalls(s)).toEqual(['newFolder']),
	newFile: (s) => expect(fileKeyCalls(s)).toEqual(['newFile']),
	rename: (s) => expect(fileKeyCalls(s)).toEqual(['rename']),
	duplicate: (s) => expect(fileKeyCalls(s)).toEqual(['duplicate']),
	moveToTrash: (s) => expect(fileKeyCalls(s)).toEqual(['moveToTrash']),
	deletePermanently: (s) => expect(fileKeyCalls(s)).toEqual(['deletePermanently']),
	restoreFromTrash: (s) => {
		const init = press(s);
		expect(
			isRestoreKey({
				key: init.key,
				ctrlKey: !!init.ctrlKey,
				metaKey: false,
				altKey: !!init.altKey,
				shiftKey: !!init.shiftKey,
			}),
		).toBe(true);
	},
	cut: (s) => expect(fileKeyCalls(s)).toEqual(['cut']),
	copy: (s) => expect(fileKeyCalls(s)).toEqual(['copy']),
	paste: (s) => expect(fileKeyCalls(s)).toEqual(['paste']),
	copyToOtherPane: (s) => expect(fileKeyCalls(s)).toEqual(['copyToOtherPane']),
	moveToOtherPane: (s) => expect(fileKeyCalls(s)).toEqual(['moveToOtherPane']),
	undo: (s) => expect(fileKeyCalls(s)).toEqual(['undo']),
	redo: (s) => expect(fileKeyCalls(s)).toEqual(['redo']),

	newTab: (s) => expect(tabKeyCalls(s)).toEqual(['newTab']),
	closeTab: (s) => expect(tabKeyCalls(s)).toEqual(['close']),
	reopenClosedTab: (s) => expect(tabKeyCalls(s)).toEqual(['reopenClosed']),

	newWindow: (s) => {
		const newWindow = vi.fn();
		const init = press(s);
		expect(
			handleWindowKey(
				{
					key: init.key,
					ctrlKey: !!init.ctrlKey,
					metaKey: false,
					altKey: !!init.altKey,
					shiftKey: !!init.shiftKey,
				},
				{ newWindow },
			),
		).toBe(true);
		expect(newWindow).toHaveBeenCalledTimes(1);
	},
	splitView: (s) => {
		const toggleSplit = vi.fn();
		const init = press(s);
		expect(
			handlePairKey(
				{
					key: init.key,
					ctrlKey: !!init.ctrlKey,
					metaKey: false,
					altKey: !!init.altKey,
					shiftKey: !!init.shiftKey,
				},
				{ toggleSplit, focusPane: vi.fn() },
				1,
				false,
			),
		).toBe(true);
		expect(toggleSplit).toHaveBeenCalledWith(1);
	},
	batchRename: (s) =>
		expect(isBatchRenameKey(new KeyboardEvent('keydown', keyEventInit(s)))).toBe(true),
	commandPalette: (s) =>
		expect(isPaletteKey({ ...keyEventInit(s), isComposing: false } as never)).toBe(true),
	help: (s) =>
		expect(helpPageForKey({ ...keyEventInit(s), isComposing: false, target: null } as never)).toBe(
			'help',
		),
	keyboardShortcuts: (s) =>
		expect(helpPageForKey({ ...keyEventInit(s), isComposing: false, target: null } as never)).toBe(
			'shortcuts',
		),
	settings: (s) =>
		expect(isSettingsShortcut({ ...keyEventInit(s), isComposing: false } as never)).toBe(true),

	viewGrid: (s) => {
		const store = createViewStore({ mode: 'list' });
		renderHook(() => useViewShortcuts(store));
		dispatchOnWindow(s);
		expect(store.getState().mode).toBe('grid');
	},
	viewList: (s) => {
		const store = createViewStore({ mode: 'grid' });
		renderHook(() => useViewShortcuts(store));
		dispatchOnWindow(s);
		expect(store.getState().mode).toBe('list');
	},
	refresh: (s) => {
		const refresh = vi.fn();
		const stop = startRefreshShortcut(window, refresh);
		dispatchOnWindow(s);
		stop();
		expect(refresh).toHaveBeenCalledTimes(1);
	},
	showHidden: (s) => {
		const store = createViewStore({ showHidden: false });
		renderHook(() => useViewShortcuts(store));
		dispatchOnWindow(s);
		expect(store.getState().showHidden).toBe(true);
	},
	sidebar: (s) => {
		const store = createSidebarStore({ open: true });
		renderHook(() => useSidebarShortcuts(store, undefined, () => {}));
		dispatchOnWindow(s);
		expect(store.getState().open).toBe(false);
	},
	toggleShelf: (s) => {
		const store = createShelfStore({ open: false });
		renderHook(() => useShelfShortcuts(() => store.getState().toggleOpen()));
		dispatchOnWindow(s);
		expect(store.getState().open).toBe(true);
	},
	toggleInspector: (s) => {
		const store = createInspectorStore({ open: false });
		renderHook(() => useInspectorShortcuts(store));
		dispatchOnWindow(s);
		expect(store.getState().open).toBe(true);
	},
	propertiesInWindow: (s) =>
		expect(isPropertiesWindowKey(new KeyboardEvent('keydown', keyEventInit(s)))).toBe(true),
	selectAll: async (s) => {
		const h = await commandsHarness();
		await select(h, 0);
		const { result } = renderHook(() =>
			useListInteractions({
				session: h.session,
				itemId: (position) => `row-${position}`,
				shown: 3,
				move: () => null,
				scrollTo: () => {},
				onOpen: undefined,
				onMenu: undefined,
			}),
		);
		act(() => result.current.onKeyDown(listKey(s)));
		expect(h.session.store.getState().selection).toEqual({ kind: 'allExcept', ids: new Set() });
	},
	invertSelection: async (s) => {
		const h = await commandsHarness();
		await select(h, 0);
		const { result } = renderHook(() =>
			useListInteractions({
				session: h.session,
				itemId: (position) => `row-${position}`,
				shown: 3,
				move: () => null,
				scrollTo: () => {},
				onOpen: undefined,
				onMenu: undefined,
			}),
		);
		act(() => result.current.onKeyDown(listKey(s)));
		// One of three was selected; the inverse is everything but that one.
		expect(h.session.store.getState().selection.kind).toBe('allExcept');
	},
};

describe('the shortcuts the registry shows', () => {
	// Each key probe mounts a hook on the window; a left-over one would take the next probe's key.
	afterEach(cleanup);

	it('parses what the registry displays into the key event that presses it', () => {
		expect(parseShortcut('Ctrl+Shift+Z')).toEqual({
			key: 'z',
			ctrl: true,
			shift: true,
			alt: false,
		});
		expect(parseShortcut('Shift+F7')).toEqual({ key: 'F7', ctrl: false, shift: true, alt: false });
		expect(parseShortcut('Ctrl+,')).toEqual({ key: ',', ctrl: true, shift: false, alt: false });
		expect(keyEventInit('Ctrl+Shift+D')).toMatchObject({ key: 'D', ctrlKey: true, shiftKey: true });
	});

	it('has a probe for every command that displays one', () => {
		const missing = COMMANDS.filter((command) => command.shortcut && !probes[command.id]).map(
			(command) => command.id,
		);
		expect(missing).toEqual([]);
	});

	for (const command of COMMANDS.filter((candidate) => candidate.shortcut)) {
		it(`${command.id} (${command.shortcut}) is the key the hooks bind`, async () => {
			await probes[command.id]!(command.shortcut!);
		});
	}
});
