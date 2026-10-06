// Draws a virtualised view's scroll step in the frame it happens in, without React's flushSync-during-render errors
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useCallback, useReducer, useRef } from 'react';
import { flushSync } from 'react-dom';

/**
 * A function for a virtualiser's `onChange`, with `useFlushSync: false`. The virtualiser's own update
 * is batched, so it lands after the frame is painted: every frame of a fast scroll showed the rows of
 * the frame before and blank space past the overscan (#565). This one renders the view from a
 * microtask queued by the scroll callback instead, which runs before the frame is painted and never
 * inside a render (what the default synchronous flush got wrong when a callback landed in one).
 * `sync` is the virtualiser's own flag: true while scrolling.
 */
export function useScrollStepRender(): (sync: boolean) => void {
	const [, rerender] = useReducer((tick: number) => tick + 1, 0);
	const queued = useRef(false);
	return useCallback((sync: boolean) => {
		if (!sync || queued.current) return;
		queued.current = true;
		queueMicrotask(() => {
			queued.current = false;
			flushSync(rerender);
		});
	}, []);
}
