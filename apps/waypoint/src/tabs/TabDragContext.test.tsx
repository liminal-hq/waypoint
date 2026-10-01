// Verifies the tear-off hook follows its factory when the factory's inputs change
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { render } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { TabDragProvider, useTearOffHook, type TearOffFactory } from './TabDragContext';
import type { TearOffHook } from './tabDrag';

describe('TabDragProvider tear-off hook', () => {
	it('uses the newest factory, resetting and reconnecting when it changes', () => {
		const point = { x: 0, y: 0 };
		const seen: { current?: TearOffHook } = {};
		function Probe() {
			seen.current = useTearOffHook();
			return null;
		}
		const disconnectA = vi.fn();
		const disconnectB = vi.fn();
		const updateA = vi.fn(() => null);
		const updateB = vi.fn(() => null);
		const a: TearOffFactory = () => ({ update: updateA, connect: () => disconnectA });
		const b: TearOffFactory = () => ({ update: updateB, connect: () => disconnectB });
		const view = render(
			<TabDragProvider tearOff={a}>
				<Probe />
			</TabDragProvider>,
		);
		const stable = seen.current!;
		stable.update!(point, {} as never);
		expect(updateA).toHaveBeenCalledTimes(1);
		view.rerender(
			<TabDragProvider tearOff={b}>
				<Probe />
			</TabDragProvider>,
		);
		expect(disconnectA).toHaveBeenCalledTimes(1);
		// The hook a drag already holds reaches the new one.
		expect(seen.current).toBe(stable);
		stable.update!(point, {} as never);
		expect(updateB).toHaveBeenCalledTimes(1);
		view.unmount();
		expect(disconnectB).toHaveBeenCalledTimes(1);
	});
});
