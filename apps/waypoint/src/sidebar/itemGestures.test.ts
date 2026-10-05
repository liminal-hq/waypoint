// Verifies which sidebar items start a drag to a split pane, and that the click ending a drag is not a click
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { PointerEvent } from 'react';
import { describe, expect, it, vi } from 'vitest';
import type { FileDragApi } from '../dnd/FileDragContext';
import { fileLocation } from '../services/fakeVfsClient';
import { itemGestures, type ItemActions, type ItemKind } from './itemGestures';

const LOCATION = fileLocation('/home/test/Documents');

function setup(kind: ItemKind, drag: Partial<FileDragApi> | null = {}) {
	const api = drag && {
		press: vi.fn(() => true),
		pressLocations: vi.fn(() => true),
		deferMenu: vi.fn(() => false),
		consumeClick: vi.fn(() => false),
		...drag,
	};
	const actions: ItemActions = { open: vi.fn(), openInNewTab: vi.fn(), openMenu: vi.fn() };
	const gestures = itemGestures(
		actions,
		{ kind, location: LOCATION, label: 'Documents' },
		{ drag: api as FileDragApi | null },
	);
	const down = (init: Partial<PointerEvent<HTMLElement>> = {}) =>
		gestures.onPointerDown({
			pointerId: 7,
			button: 0,
			clientX: 12,
			clientY: 34,
			currentTarget: document.createElement('div'),
			nativeEvent: { ctrlKey: false, shiftKey: false, altKey: false },
			...init,
		} as PointerEvent<HTMLElement>);
	return { api, actions, gestures, down };
}

describe('a sidebar item as a drag source', () => {
	it.each(['place', 'folder'] as const)(
		'a %s starts a drag of its folder, marked as a place',
		(kind) => {
			const { api, down } = setup(kind);
			down();
			expect(api!.pressLocations).toHaveBeenCalledWith(
				expect.objectContaining({
					pointerId: 7,
					button: 0,
					locations: [LOCATION],
					name: 'Documents',
					folder: null,
					place: true,
				}),
			);
		},
	);

	it.each(['trash', 'favourite'] as const)('a %s does not (it has its own meaning)', (kind) => {
		const { api, down } = setup(kind);
		down();
		expect(api!.pressLocations).not.toHaveBeenCalled();
	});

	it('only the primary button drags, and nothing drags without a window drag', () => {
		const { api, down } = setup('place');
		down({ button: 2 });
		down({ button: 1 });
		expect(api!.pressLocations).not.toHaveBeenCalled();
		expect(() => setup('place', null).down()).not.toThrow();
	});

	it('the click that ends a drag does not open the item', () => {
		const ended = setup('place', { consumeClick: vi.fn(() => true) });
		ended.gestures.onClick({ button: 0 } as never);
		expect(ended.actions.open).not.toHaveBeenCalled();
		const plain = setup('place');
		plain.gestures.onClick({ button: 0 } as never);
		expect(plain.actions.open).toHaveBeenCalledWith(LOCATION);
	});
});
