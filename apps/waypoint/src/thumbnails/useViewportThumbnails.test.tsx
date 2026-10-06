// Verifies that a view asks for thumbnails at once when still, and only once a scroll settles while it moves
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ThumbnailLoader } from './thumbnailLoader';
import { useViewportThumbnails } from './useViewportThumbnails';

type Item = { key: string };

function setup(settleMs: number) {
	const want = vi.fn();
	const loader = { want } as unknown as ThumbnailLoader<Item>;
	const view = renderHook(
		({ first, scrolling }: { first: number; scrolling?: boolean }) =>
			useViewportThumbnails<Item>({
				loader,
				first,
				last: first + 9,
				count: 100_000,
				version: 1,
				itemAt: (position) => ({ key: `k${position}` }),
				settleMs,
				scrolling,
			}),
		{ initialProps: { first: 0 } as { first: number; scrolling?: boolean } },
	);
	return { want, view };
}

const firstVisible = (want: ReturnType<typeof vi.fn>, call: number) =>
	(want.mock.calls[call]![0] as Item[])[0]!.key;

beforeEach(() => {
	vi.useFakeTimers();
});
afterEach(() => {
	vi.useRealTimers();
});

describe('thumbnails for a view', () => {
	it('asks at every change without a settle time', () => {
		const { want, view } = setup(0);
		for (let first = 10; first <= 50; first += 10) view.rerender({ first });
		expect(want).toHaveBeenCalledTimes(6);
	});

	it('asks at once after a quiet spell, and only for where a scroll settles while it moves', () => {
		const { want, view } = setup(120);
		expect(want).toHaveBeenCalledTimes(1);
		expect(firstVisible(want, 0)).toBe('k0');
		// A scroll: a new range every frame, none of them asked for while it lasts.
		for (let frame = 1; frame <= 30; frame++) {
			act(() => vi.advanceTimersByTime(16));
			view.rerender({ first: frame * 40 });
		}
		expect(want).toHaveBeenCalledTimes(1);
		act(() => vi.advanceTimersByTime(120));
		expect(want).toHaveBeenCalledTimes(2);
		expect(firstVisible(want, 1)).toBe('k1200');
		// Still again: the next change is asked for straight away.
		act(() => vi.advanceTimersByTime(500));
		view.rerender({ first: 2000 });
		expect(want).toHaveBeenCalledTimes(3);
	});

	it('waits for a scroll that starts after a quiet spell to settle too', () => {
		const { want, view } = setup(120);
		act(() => vi.advanceTimersByTime(1000));
		// A jump (End, the scrollbar) is a scroll: what it lands on is asked for once it has stopped.
		view.rerender({ first: 5000, scrolling: true });
		expect(want).toHaveBeenCalledTimes(1);
		act(() => vi.advanceTimersByTime(120));
		expect(want).toHaveBeenCalledTimes(2);
		expect(firstVisible(want, 1)).toBe('k5000');
	});

	it('drops a request still waiting when the view goes', () => {
		const { want, view } = setup(120);
		act(() => vi.advanceTimersByTime(16));
		view.rerender({ first: 40 });
		view.unmount();
		act(() => vi.advanceTimersByTime(500));
		expect(want).toHaveBeenCalledTimes(1);
	});
});
