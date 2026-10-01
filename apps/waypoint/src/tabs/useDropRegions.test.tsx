// Verifies the drop regions stay registered across layout changes and wait for a strip that is not mounted yet
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FakeTearoffClient } from '../services/fakeTearoffClient';
import { REGION_DELAY_MS, useDropRegions } from './useTearoff';

function mountStrip(tabs: number) {
	const strip = document.createElement('div');
	strip.setAttribute('data-strip', '');
	const list = document.createElement('div');
	list.setAttribute('role', 'tablist');
	for (let i = 0; i < tabs; i++) {
		const slot = document.createElement('div');
		slot.setAttribute('data-slot', '');
		slot.setAttribute('data-index', String(i));
		list.append(slot);
	}
	strip.append(list);
	document.body.append(strip);
	return strip;
}

beforeEach(() => {
	vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (
		this: HTMLElement,
	) {
		const index = Number(this.getAttribute('data-index') ?? 0);
		const left = this.hasAttribute('data-strip') ? 0 : index * 100;
		const right = this.hasAttribute('data-strip') ? 800 : left + 100;
		return { left, right, top: 0, bottom: 30, width: right - left, height: 30 } as DOMRect;
	});
});

afterEach(() => {
	document.body.replaceChildren();
	vi.restoreAllMocks();
});

describe('useDropRegions', () => {
	it('replaces the set on a layout change without clearing it first', () => {
		mountStrip(2);
		const client = new FakeTearoffClient({ hitTest: true });
		const sets: string[][] = [];
		vi.spyOn(client, 'setDropRegions').mockImplementation(async (regions) => {
			sets.push(regions.map((region) => region.id));
		});
		const { rerender, unmount } = renderHook(({ key }) => useDropRegions(client, true, key), {
			initialProps: { key: 'one' },
		});
		const first = sets.length;
		expect(first).toBeGreaterThan(0);
		const extra = document.createElement('div');
		extra.setAttribute('data-slot', '');
		extra.setAttribute('data-index', '2');
		document.querySelector('[role="tablist"]')!.append(extra);
		rerender({ key: 'two' });
		expect(sets.length).toBe(first + 1);
		expect(sets.every((set) => set.length > 0)).toBe(true);
		unmount();
		expect(sets.at(-1)).toEqual([]);
	});

	it('attaches its listeners when the strip appears after the hook ran', async () => {
		vi.useFakeTimers();
		try {
			const client = new FakeTearoffClient({ hitTest: true });
			const sets: string[][] = [];
			vi.spyOn(client, 'setDropRegions').mockImplementation(async (regions) => {
				sets.push(regions.map((region) => region.id));
			});
			renderHook(() => useDropRegions(client, true, 'one'));
			expect(sets.at(-1) ?? []).toEqual([]);
			await act(async () => {
				mountStrip(1);
				await vi.advanceTimersByTimeAsync(0);
				await vi.advanceTimersByTimeAsync(REGION_DELAY_MS + 1);
			});
			expect(sets.at(-1)).toEqual(['strip', 'slot:0', 'slot:1']);
		} finally {
			vi.useRealTimers();
		}
	});
});
