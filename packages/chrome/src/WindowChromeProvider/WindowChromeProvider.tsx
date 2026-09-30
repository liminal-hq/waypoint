// Window chrome provider: subscribes to the host window once and shares its state
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from 'react';
import type { WindowControls } from '../TitleBar/windowControls';

interface WindowChromeState {
	controls: WindowControls;
	maximised: boolean;
	focused: boolean;
	/** False until the adapter's initial focus read has settled. */
	focusSettled: boolean;
}

const WindowChromeContext = createContext<WindowChromeState | null>(null);

export interface WindowChromeProviderProps {
	/** Pass a stable object: the provider resubscribes when its identity changes. */
	controls: WindowControls;
	children: ReactNode;
}

/**
 * Reads the initial maximised and focus state from the adapter, follows changes, and shares
 * `{ controls, maximised, focused }` with every chrome component below it. A host that cannot
 * report focus is treated as always focused.
 */
export function WindowChromeProvider({ controls, children }: WindowChromeProviderProps) {
	const [maximised, setMaximised] = useState(false);
	const [focused, setFocused] = useState(true);
	const [focusSettled, setFocusSettled] = useState(!controls.isFocused);

	useEffect(() => {
		let active = true;
		Promise.resolve(controls.isMaximized())
			.then((value) => {
				if (active) setMaximised(value);
			})
			.catch(() => {});
		if (controls.isFocused) {
			Promise.resolve(controls.isFocused())
				.then((value) => {
					if (active) setFocused(value);
				})
				.catch(() => {})
				.finally(() => {
					if (active) setFocusSettled(true);
				});
		}
		const unsubscribeMaximised = controls.onMaximizedChange((value) => {
			if (active) setMaximised(value);
		});
		const unsubscribeFocus = controls.onFocusChange?.((value) => {
			if (active) {
				setFocused(value);
				setFocusSettled(true);
			}
		});
		return () => {
			active = false;
			unsubscribeMaximised();
			unsubscribeFocus?.();
		};
	}, [controls]);

	const value = useMemo(
		() => ({ controls, maximised, focused, focusSettled }),
		[controls, maximised, focused, focusSettled],
	);
	return <WindowChromeContext.Provider value={value}>{children}</WindowChromeContext.Provider>;
}

function useRequiredChrome(): WindowChromeState {
	const state = useContext(WindowChromeContext);
	if (!state) {
		throw new Error(
			'Window chrome components must be rendered inside a <WindowChromeProvider controls={…}>.',
		);
	}
	return state;
}

/** The host window adapter. Throws outside a `WindowChromeProvider`. */
export function useWindowControls(): WindowControls {
	return useRequiredChrome().controls;
}

/** Whether the window is maximised. Throws outside a `WindowChromeProvider`. */
export function useWindowMaximised(): boolean {
	return useRequiredChrome().maximised;
}

/** Whether the window is focused. Throws outside a `WindowChromeProvider`. */
export function useWindowFocused(): boolean {
	return useRequiredChrome().focused;
}

/**
 * Focus state for consumers that must also work without a provider: `undefined` without one, so
 * callers can fall back to DOM events. `settled` is false until the initial read has resolved,
 * which lets a caller tell that first read apart from a real focus loss.
 */
export function useOptionalWindowFocus(): { focused: boolean; settled: boolean } | undefined {
	const state = useContext(WindowChromeContext);
	return state ? { focused: state.focused, settled: state.focusSettled } : undefined;
}
