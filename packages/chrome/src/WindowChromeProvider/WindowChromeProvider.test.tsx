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

describe('WindowChromeProvider initial state and change events', () => {
	function StateProbe() {
		return (
			<p data-testid="state">
				{String(useWindowMaximised())}/{String(useWindowFocused())}
			</p>
		);
	}

	function deferredControls() {
		const events = {
			maximised: (_value: boolean) => {},
			focus: (_value: boolean) => {},
			finishMaximised: (_value: boolean) => {},
			finishFocused: (_value: boolean) => {},
		};
		const log: string[] = [];
		const controls: WindowControls = {
			minimize: () => {},
			toggleMaximize: () => {},
			close: () => {},
			setAlwaysOnTop: () => {},
			isMaximized: () => {
				log.push('read maximised');
				return new Promise<boolean>((resolve) => (events.finishMaximised = resolve));
			},
			onMaximizedChange: (listener) => {
				log.push('subscribe maximised');
				events.maximised = listener;
				return Object.assign(() => {}, { ready: Promise.resolve() });
			},
			isFocused: () => {
				log.push('read focused');
				return new Promise<boolean>((resolve) => (events.finishFocused = resolve));
			},
			onFocusChange: (listener) => {
				log.push('subscribe focus');
				events.focus = listener;
				return Object.assign(() => {}, { ready: Promise.resolve() });
			},
		};
		return { controls, events, log };
	}

	it('does not let an older initial read overwrite a newer change event', async () => {
		const { controls, events } = deferredControls();
		render(
			<WindowChromeProvider controls={controls}>
				<StateProbe />
			</WindowChromeProvider>,
		);
		await act(async () => {});
		act(() => {
			events.maximised(true); // newer: maximised
			events.focus(false); // newer: lost focus
		});
		await act(async () => {
			events.finishMaximised(false); // older snapshots land last
			events.finishFocused(true);
		});
		expect(screen.getByTestId('state').textContent).toBe('true/false');
	});

	it('takes the initial read when no change event arrived first', async () => {
		const { controls, events } = deferredControls();
		render(
			<WindowChromeProvider controls={controls}>
				<StateProbe />
			</WindowChromeProvider>,
		);
		await act(async () => {});
		await act(async () => {
			events.finishMaximised(true);
			events.finishFocused(false);
		});
		expect(screen.getByTestId('state').textContent).toBe('true/false');
	});

	it('reads only after the listeners are live, so a change cannot slip between them', async () => {
		const log: string[] = [];
		let goLive: () => void = () => {};
		const live = new Promise<void>((resolve) => (goLive = resolve));
		const controls: WindowControls = {
			minimize: () => {},
			toggleMaximize: () => {},
			close: () => {},
			setAlwaysOnTop: () => {},
			isMaximized: () => {
				log.push('read');
				return false;
			},
			onMaximizedChange: () => {
				log.push('subscribe');
				return Object.assign(() => {}, { ready: live });
			},
		};
		render(
			<WindowChromeProvider controls={controls}>
				<StateProbe />
			</WindowChromeProvider>,
		);
		await act(async () => {});
		expect(log).toEqual(['subscribe']);
		await act(async () => goLive());
		expect(log).toEqual(['subscribe', 'read']);
	});
});
