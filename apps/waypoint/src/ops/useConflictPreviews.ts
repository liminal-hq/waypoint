// Loads the previews of a conflict dialog's clashes: the first few by themselves, a few at a time, and any other when it is asked for
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Conflict } from '@liminal-hq/waypoint-protocol/generated/Conflict';
import type { ConflictPreview } from '@liminal-hq/waypoint-protocol/generated/ConflictPreview';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { IDLE, PREVIEW_CONCURRENCY, canCompare, type PreviewState } from './conflictPreviewModel';

export interface ConflictPreviews {
	/** Whether previews can be asked for at all. */
	available: boolean;
	stateOf(conflict: Conflict): PreviewState;
	/** Asks for one now, ahead of the others waiting. */
	request(conflict: Conflict): void;
}

/**
 * `load` is `undefined` where the client cannot preview, and then nothing is ever asked for and
 * every state stays idle. A failed load is only `failed`: the dialog works without it. Answers
 * that arrive after the dialog has gone are dropped.
 */
export function useConflictPreviews(
	load: ((conflict: Conflict) => Promise<ConflictPreview>) | undefined,
	auto: readonly Conflict[],
): ConflictPreviews {
	const [states, setStates] = useState<ReadonlyMap<string, PreviewState>>(new Map());
	const loadRef = useRef(load);
	loadRef.current = load;
	const queue = useRef<Conflict[]>([]);
	const asked = useRef(new Set<string>());
	const running = useRef(0);
	const alive = useRef(true);

	useEffect(() => {
		alive.current = true;
		return () => {
			alive.current = false;
		};
	}, []);

	const pump = useCallback(() => {
		while (running.current < PREVIEW_CONCURRENCY) {
			const next = queue.current.shift();
			const loader = loadRef.current;
			if (!next || !loader) return;
			running.current += 1;
			const key = next.source.uri;
			setStates((now) => new Map(now).set(key, { status: 'loading' }));
			const settle = (state: PreviewState) => {
				running.current -= 1;
				if (alive.current) setStates((now) => new Map(now).set(key, state));
				pump();
			};
			let work: Promise<ConflictPreview>;
			try {
				work = loader(next);
			} catch {
				settle({ status: 'failed' });
				continue;
			}
			work.then(
				(preview) => settle({ status: 'ready', preview }),
				() => settle({ status: 'failed' }),
			);
		}
	}, []);

	const enqueue = useCallback(
		(conflict: Conflict, front: boolean) => {
			if (!loadRef.current || !canCompare(conflict) || asked.current.has(conflict.source.uri)) {
				return;
			}
			asked.current.add(conflict.source.uri);
			if (front) queue.current.unshift(conflict);
			else queue.current.push(conflict);
			pump();
		},
		[pump],
	);

	const autoKey = auto.map((conflict) => conflict.source.uri).join('\n');
	useEffect(() => {
		for (const conflict of auto) enqueue(conflict, false);
		// `auto` is read for the keys it holds, which `autoKey` stands for.
	}, [autoKey, enqueue]);

	const stateOf = useCallback(
		(conflict: Conflict) => states.get(conflict.source.uri) ?? IDLE,
		[states],
	);
	const request = useCallback((conflict: Conflict) => enqueue(conflict, true), [enqueue]);
	return useMemo(
		() => ({ available: load !== undefined, stateOf, request }),
		[load, stateOf, request],
	);
}
