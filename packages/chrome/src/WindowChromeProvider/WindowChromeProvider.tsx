// Window chrome provider: subscribes to the host window once and shares its state
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import {
	createContext,
	useCallback,
	useContext,
	useEffect,
	useMemo,
	useRef,
	useState,
	type ReactNode,
} from 'react';
import type { WindowControls } from '../TitleBar/windowControls';

interface WindowChromeState {
	controls: WindowControls;
	maximised: boolean;
	/** Shows the expected maximised state at once and confirms it against the window shortly after. */
	expectMaximised: (value: boolean) => void;
	focused: boolean;
	/** False until the adapter's initial focus read has settled. */
	focusSettled: boolean;
}

/**
 * How long an expected maximised state stands before it is checked against the window. The host's
 * own resize event normally corrects it sooner; this covers a toggle that changed nothing.
 */
const EXPECTATION_CHECK_MS = 400;

const WindowChromeContext = createContext<WindowChromeState | null>(null);

export interface WindowChromeProviderProps {
	/** Pass a stable object: the provider resubscribes when its identity changes. */
	controls: WindowControls;
	children: ReactNode;
}

/**
 * Follows the host window's maximised and focus state and shares
 * `{ controls, maximised, focused }` with every chrome component below it. A host that cannot
 * report focus is treated as always focused.
 */
export function WindowChromeProvider({ controls, children }: WindowChromeProviderProps) {
	const [maximised, setMaximised] = useState(false);
	const [focused, setFocused] = useState(true);
	const [focusSettled, setFocusSettled] = useState(!controls.isFocused);
	const checkTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

	useEffect(() => {
		let active = true;
		// A change event is newer than any read that was still in flight when it arrived, so a read
		// that resolves after its event is discarded rather than allowed to overwrite it.
		let maximisedChanged = false;
		let focusChanged = false;

		const unsubscribeMaximised = controls.onMaximizedChange((value) => {
			if (!active) return;
			maximisedChanged = true;
			setMaximised(value);
		});
		const unsubscribeFocus = controls.onFocusChange?.((value) => {
			if (!active) return;
			focusChanged = true;
			setFocused(value);
			setFocusSettled(true);
		});

		// Subscribe first, read second: only once the listeners are live can a change no longer slip
		// between the read and the subscription.
		void Promise.all([unsubscribeMaximised.ready, unsubscribeFocus?.ready]).then(() => {
			if (!active) return;
			Promise.resolve(controls.isMaximized())
				.then((value) => {
					if (active && !maximisedChanged) setMaximised(value);
				})
				.catch(() => {});
			if (controls.isFocused) {
				Promise.resolve(controls.isFocused())
					.then((value) => {
						if (active && !focusChanged) setFocused(value);
					})
					.catch(() => {})
					.finally(() => {
						if (active) setFocusSettled(true);
					});
			}
		});

		return () => {
			active = false;
			clearTimeout(checkTimer.current);
			unsubscribeMaximised();
			unsubscribeFocus?.();
		};
	}, [controls]);

	// The host reports a maximise only after the window has already resized, which leaves a moment
	// where a full-screen window still draws its shadow margin. A toggle the chrome itself asks for
	// is shown straight away, then checked against the window so a toggle that did nothing is undone.
	const expectMaximised = useCallback(
		(value: boolean) => {
			setMaximised(value);
			clearTimeout(checkTimer.current);
			checkTimer.current = setTimeout(() => {
				Promise.resolve(controls.isMaximized())
					.then(setMaximised)
					.catch(() => {});
			}, EXPECTATION_CHECK_MS);
		},
		[controls],
	);

	const value = useMemo(
		() => ({ controls, maximised, expectMaximised, focused, focusSettled }),
		[controls, maximised, expectMaximised, focused, focusSettled],
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

/**
 * Shows a maximised state the chrome has just asked for before the host reports it. Throws
 * outside a `WindowChromeProvider`.
 */
export function useExpectMaximised(): (value: boolean) => void {
	return useRequiredChrome().expectMaximised;
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
