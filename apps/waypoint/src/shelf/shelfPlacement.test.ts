// Verifies where the Shelf is shown: the dock or the window, the toggles for each, and the moves between them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import { FakeShelfWindowClient } from '../services/fakeShelfWindowClient';
import { createShelfPlacement } from './shelfPlacement';
import { createShelfStore, type ShelfStore } from './shelfStore';

function setup(options: { inWindow?: boolean; undocked?: boolean; reject?: boolean } = {}) {
	const store: ShelfStore = createShelfStore({
		open: options.inWindow ?? false,
		undocked: options.undocked ?? options.inWindow ?? false,
	});
	const client = new FakeShelfWindowClient();
	const api = {
		setShelfUndocked: vi.fn(async (_undocked: boolean) => {
			if (options.reject) throw 'could not create the window: boom';
		}),
	};
	const say = vi.fn();
	const announce = vi.fn();
	const placement = createShelfPlacement({
		store,
		api,
		client,
		inWindow: options.inWindow ?? false,
		say,
		announce,
	});
	return { store, client, api, say, announce, placement };
}

describe('toggle', () => {
	it('shows and hides the dock while the Shelf is docked, and leaves the window alone', () => {
		const { store, client, placement } = setup();
		placement.toggle();
		expect(store.getState().open).toBe(true);
		placement.toggle();
		expect(store.getState().open).toBe(false);
		expect(client.calls).toEqual([]);
	});

	it('raises or hides the Shelf window while the Shelf is undocked, and leaves the dock alone', () => {
		const { store, client, placement } = setup({ undocked: true });
		placement.toggle();
		expect(client.calls).toEqual(['toggle']);
		expect(store.getState().open).toBe(false);
	});

	it('hides the Shelf window from inside it', () => {
		const { client, placement } = setup({ inWindow: true });
		placement.toggle();
		expect(client.calls).toEqual(['toggle']);
	});

	it('says so when the window could not be shown or hidden', async () => {
		const { client, say, placement } = setup({ undocked: true });
		client.toggle = async () => {
			throw new Error('no window');
		};
		placement.toggle();
		await vi.waitFor(() =>
			expect(say).toHaveBeenCalledWith('Could not show or hide the Shelf window'),
		);
	});
});

describe('focus', () => {
	it('opens the dock and asks for the focus while docked', () => {
		const { store, placement } = setup();
		placement.focus();
		expect(store.getState().open).toBe(true);
		expect(store.getState().focusRequests).toBe(1);
	});

	it('raises the window from a main window while undocked, and asks for the focus inside the window', () => {
		const outside = setup({ undocked: true });
		outside.placement.focus();
		expect(outside.client.calls).toEqual(['raise']);
		const inside = setup({ inWindow: true });
		inside.placement.focus();
		expect(inside.store.getState().focusRequests).toBe(1);
		expect(inside.client.calls).toEqual([]);
	});
});

describe('undock and dock', () => {
	it('undocks through the session and announces it', async () => {
		const { api, announce, placement } = setup();
		await placement.undock();
		expect(api.setShelfUndocked).toHaveBeenCalledWith(true);
		expect(announce).toHaveBeenCalledWith('The Shelf is in its own window');
	});

	it('docks through the session and announces it', async () => {
		const { api, announce, placement } = setup({ undocked: true });
		await placement.dock();
		expect(api.setShelfUndocked).toHaveBeenCalledWith(false);
		expect(announce).toHaveBeenCalledWith('The Shelf is docked');
	});

	it('does nothing when the Shelf is already where it was asked to go', async () => {
		const docked = setup();
		await docked.placement.dock();
		const undocked = setup({ undocked: true });
		await undocked.placement.undock();
		expect(docked.api.setShelfUndocked).not.toHaveBeenCalled();
		expect(undocked.api.setShelfUndocked).not.toHaveBeenCalled();
	});

	it('tells the person when the window cannot be made, and announces nothing', async () => {
		const { say, announce, placement } = setup({ reject: true });
		vi.spyOn(console, 'warn').mockImplementation(() => {});
		await placement.undock();
		expect(say).toHaveBeenCalledWith('Could not open the Shelf in its own window');
		expect(announce).not.toHaveBeenCalled();
	});
});
