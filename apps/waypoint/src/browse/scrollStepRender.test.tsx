// Verifies that a scroll step renders the view from a microtask, once however many callbacks ask
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { useScrollStepRender } from './scrollStepRender';

describe('drawing a scroll step', () => {
	it('renders before the frame (in a microtask, not a task), once for a burst, and not when the scroll ends', async () => {
		let renders = 0;
		const view = renderHook(() => {
			renders += 1;
			return useScrollStepRender();
		});
		const drawStep = view.result.current;
		const before = renders;
		const task = vi.fn();
		await act(async () => {
			setTimeout(task, 0);
			drawStep(true);
			drawStep(true);
			drawStep(false);
			await Promise.resolve();
			// Drawn already, and before any task (the frame's paint comes after the microtasks).
			expect(renders).toBe(before + 1);
			expect(task).not.toHaveBeenCalled();
		});
		drawStep(false);
		await act(async () => {
			await Promise.resolve();
		});
		expect(renders).toBe(before + 1);
		expect(view.result.current).toBe(drawStep);
	});
});
