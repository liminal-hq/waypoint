// Lets an icon that is still arriving (the system's picture, a server's state) say so, so whoever draws it can wait for it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useContext, useEffect } from 'react';

/** Counts the icons under it that are not yet what they will be, and says when none is left. */
export interface IconSettleTracker {
	/** Marks one icon as still arriving; the returned function marks it done. */
	hold(): () => void;
	/** How many icons are still arriving. */
	pending(): number;
	/** Resolves `true` once none is, or `false` when `timeoutMs` passes first. */
	settled(timeoutMs: number): Promise<boolean>;
}

export function createIconSettleTracker(): IconSettleTracker {
	let holds = 0;
	const waiters = new Set<() => void>();
	return {
		hold() {
			holds += 1;
			let released = false;
			return () => {
				if (released) return;
				released = true;
				holds -= 1;
				// An icon that is replaced releases before its successor holds, in the same commit.
				if (holds === 0) queueMicrotask(() => waiters.forEach((check) => check()));
			};
		},
		pending: () => holds,
		settled(timeoutMs) {
			if (holds === 0) return Promise.resolve(true);
			return new Promise<boolean>((resolve) => {
				const finish = (done: boolean) => {
					waiters.delete(check);
					window.clearTimeout(timer);
					resolve(done);
				};
				const check = () => {
					if (holds === 0) finish(true);
				};
				const timer = window.setTimeout(() => finish(false), timeoutMs);
				waiters.add(check);
			});
		},
	};
}

/** Provided only where a picture of an icon is being made (the native menu's stage); an icon anywhere else holds nothing. */
export const IconSettleContext = createContext<IconSettleTracker | null>(null);

/**
 * Holds the tracker above this icon, if there is one, for as long as `pending` is true. An icon that
 * draws a stand-in while its real picture loads calls it with whether it is still on the stand-in.
 */
export function useSettleHold(pending: boolean): void {
	const tracker = useContext(IconSettleContext);
	useEffect(() => {
		if (!tracker || !pending) return;
		return tracker.hold();
	}, [tracker, pending]);
}
