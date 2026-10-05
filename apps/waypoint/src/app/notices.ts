// A one-at-a-time notice with an optional action (an Undo toast), shown by `NoticeToast`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useSyncExternalStore } from 'react';

export interface NoticeAction {
	label: string;
	run: () => void;
}

export interface Notice {
	/** Numbered, so the same message arriving again restarts its timer. */
	id: number;
	text: string;
	action?: NoticeAction;
	/** Further buttons after `action` (Resume, then Discard…). */
	more?: NoticeAction[];
	/** Called once when the notice goes away by itself or is dismissed (not when another replaces it). */
	onClose?: () => void;
}

/** More of a notice: further buttons, and what to do once it has gone. */
export interface NoticeExtras {
	more?: NoticeAction[];
	onClose?: () => void;
}

let current: Notice | null = null;
let counter = 0;
const listeners = new Set<() => void>();

function emit(): void {
	listeners.forEach((listener) => listener());
}

/** Shows `text` (replacing any notice already up) with an optional action button. */
export function showNotice(text: string, action?: NoticeAction, extras: NoticeExtras = {}): number {
	counter += 1;
	current = {
		id: counter,
		text,
		...(action ? { action } : {}),
		...(extras.more ? { more: extras.more } : {}),
		...(extras.onClose ? { onClose: extras.onClose } : {}),
	};
	emit();
	return current.id;
}

/** Hides the notice, or only the one numbered `id` (so a late timer cannot hide a newer message). */
export function dismissNotice(id?: number): void {
	if (current === null || (id !== undefined && current.id !== id)) return;
	const closed = current;
	current = null;
	emit();
	closed.onClose?.();
}

export function useNotice(): Notice | null {
	return useSyncExternalStore(
		(listener) => {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
		() => current,
	);
}
