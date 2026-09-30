// Verifies the window frame follows the maximised state and passes content through
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { WindowFrame } from './WindowFrame';

function controls(initiallyMaximised: boolean) {
	let listener: (value: boolean) => void = () => {};
	return {
		isMaximized: vi.fn().mockResolvedValue(initiallyMaximised),
		onMaximizedChange: (l: (value: boolean) => void) => {
			listener = l;
			return () => {};
		},
		emit: (value: boolean) => listener(value),
	};
}

describe('WindowFrame', () => {
	it('renders its children with the caller class and starts rounded', () => {
		render(
			<WindowFrame windowControls={controls(false)} className="extra">
				<p>inside</p>
			</WindowFrame>,
		);
		const surface = screen.getByText('inside').parentElement!;
		const frame = surface.parentElement!;
		expect(surface.classList.contains('extra')).toBe(true);
		expect(frame.getAttribute('data-maximised')).toBe('false');
	});

	it('goes square when the window is already maximised', async () => {
		render(
			<WindowFrame windowControls={controls(true)}>
				<p>inside</p>
			</WindowFrame>,
		);
		const frame = screen.getByText('inside').parentElement!.parentElement!;
		await waitFor(() => expect(frame.getAttribute('data-maximised')).toBe('true'));
	});

	it('follows maximise and restore', async () => {
		const c = controls(false);
		render(
			<WindowFrame windowControls={c}>
				<p>inside</p>
			</WindowFrame>,
		);
		const frame = screen.getByText('inside').parentElement!.parentElement!;
		// Let the initial read settle so it cannot land after the event below.
		await act(async () => {});
		act(() => c.emit(true));
		expect(frame.getAttribute('data-maximised')).toBe('true');
		act(() => c.emit(false));
		expect(frame.getAttribute('data-maximised')).toBe('false');
	});
});
