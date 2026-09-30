// Tests that the provider subscribes once and shares window state with every consumer
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { ContextMenu } from '../ContextMenu/ContextMenu';
import { TitleBar } from '../TitleBar/TitleBar';
import type { WindowControls } from '../TitleBar/windowControls';
import { WindowFrame } from '../WindowFrame/WindowFrame';
import {
	useWindowControls,
	useWindowFocused,
	useWindowMaximised,
	WindowChromeProvider,
} from './WindowChromeProvider';

function fakeControls() {
	let onMax: (value: boolean) => void = () => {};
	let onFocus: (value: boolean) => void = () => {};
	const unsubscribeMaximised = vi.fn();
	const unsubscribeFocus = vi.fn();
	const controls: WindowControls = {
		minimize: vi.fn(),
		toggleMaximize: vi.fn(),
		close: vi.fn(),
		setAlwaysOnTop: vi.fn(),
		isMaximized: vi.fn(async () => false),
		onMaximizedChange: vi.fn((listener) => {
			onMax = listener;
			return unsubscribeMaximised;
		}),
		isFocused: vi.fn(async () => true),
		onFocusChange: vi.fn((listener) => {
			onFocus = listener;
			return unsubscribeFocus;
		}),
	};
	return {
		controls,
		unsubscribeMaximised,
		unsubscribeFocus,
		emitMaximised: (value: boolean) => act(() => onMax(value)),
		emitFocused: (value: boolean) => act(() => onFocus(value)),
	};
}

function Probe() {
	const controls = useWindowControls();
	const maximised = useWindowMaximised();
	const focused = useWindowFocused();
	return (
		<output data-testid="probe" data-same={String(controls !== undefined)}>
			{`${maximised}/${focused}`}
		</output>
	);
}

describe('WindowChromeProvider', () => {
	it('subscribes once however many chrome components are mounted', async () => {
		const fake = fakeControls();
		render(
			<WindowChromeProvider controls={fake.controls}>
				<TitleBar />
				<WindowFrame>
					<p>inside</p>
				</WindowFrame>
				<ContextMenu items={[]} position={{ x: 0, y: 0 }} onSelect={() => {}} onClose={() => {}} />
			</WindowChromeProvider>,
		);
		await act(async () => {});
		expect(fake.controls.onMaximizedChange).toHaveBeenCalledTimes(1);
		expect(fake.controls.onFocusChange).toHaveBeenCalledTimes(1);
		expect(fake.controls.isMaximized).toHaveBeenCalledTimes(1);
		expect(fake.controls.isFocused).toHaveBeenCalledTimes(1);
	});

	it('shares state through the hooks and follows changes', async () => {
		const fake = fakeControls();
		render(
			<WindowChromeProvider controls={fake.controls}>
				<Probe />
			</WindowChromeProvider>,
		);
		await act(async () => {});
		expect(screen.getByTestId('probe')).toHaveTextContent('false/true');
		fake.emitMaximised(true);
		fake.emitFocused(false);
		expect(screen.getByTestId('probe')).toHaveTextContent('true/false');
	});

	it('reads the initial state from the adapter', async () => {
		const fake = fakeControls();
		fake.controls.isMaximized = vi.fn(async () => true);
		fake.controls.isFocused = vi.fn(async () => false);
		render(
			<WindowChromeProvider controls={fake.controls}>
				<Probe />
			</WindowChromeProvider>,
		);
		await waitFor(() => expect(screen.getByTestId('probe')).toHaveTextContent('true/false'));
	});

	it('unsubscribes on unmount', () => {
		const fake = fakeControls();
		const { unmount } = render(
			<WindowChromeProvider controls={fake.controls}>
				<Probe />
			</WindowChromeProvider>,
		);
		unmount();
		expect(fake.unsubscribeMaximised).toHaveBeenCalledTimes(1);
		expect(fake.unsubscribeFocus).toHaveBeenCalledTimes(1);
	});

	it('treats a host without focus support as focused', async () => {
		const fake = fakeControls();
		delete fake.controls.isFocused;
		delete fake.controls.onFocusChange;
		render(
			<WindowChromeProvider controls={fake.controls}>
				<Probe />
			</WindowChromeProvider>,
		);
		await act(async () => {});
		expect(screen.getByTestId('probe')).toHaveTextContent('false/true');
	});

	it('throws from the hooks outside a provider', () => {
		const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
		expect(() => render(<Probe />)).toThrow(/WindowChromeProvider/);
		spy.mockRestore();
	});
});
