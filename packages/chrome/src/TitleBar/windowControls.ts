// Adapter contract between the presentational chrome and a host window API
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

type MaybePromise<T> = T | Promise<T>;

/** Everything the chrome needs from the host window. Tauri, tests and other shells can implement it. */
export interface WindowControls {
	minimize(): MaybePromise<void>;
	toggleMaximize(): MaybePromise<void>;
	close(): MaybePromise<void>;
	/** Begins an OS-driven window move. Enables the "Move" entry of the window menu when present. */
	startDragging?(): MaybePromise<void>;
	setAlwaysOnTop(value: boolean): MaybePromise<void>;
	/** Initial Always on Top state. Assumed false when omitted. */
	isAlwaysOnTop?(): MaybePromise<boolean>;
	/**
	 * Asks the host's window manager (compositor) to show its own window menu at a point in the
	 * window, in CSS pixels from the top-left. Enables the "More window options…" entry of the
	 * window menu when present. Resolves false when the request was refused.
	 */
	showSystemMenu?(position: { x: number; y: number }): MaybePromise<boolean | void>;
	isMaximized(): MaybePromise<boolean>;
	/** Subscribes to maximised-state changes and returns a synchronous unsubscribe function. */
	onMaximizedChange(listener: (maximised: boolean) => void): () => void;
	/** Initial focus state. The window is assumed focused when omitted. */
	isFocused?(): MaybePromise<boolean>;
	/** Subscribes to focus changes and returns a synchronous unsubscribe function. */
	onFocusChange?(listener: (focused: boolean) => void): () => void;
	/**
	 * True when the host already maximises on double-click of a drag region (Tauri does), so the
	 * title bar must not toggle a second time.
	 */
	handlesDoubleClickNatively?: boolean;
}

export type ControlsStyle = 'gnome' | 'kde' | 'win11' | 'cinnamon';
