// Verifies which pane F5 copies to: the next pane of the pair, and none when the pane is not paired or the other has no listing
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Pair } from '@liminal-hq/waypoint-protocol/generated/Pair';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import { describe, expect, it } from 'vitest';
import type { ListingSession, SessionState } from '../browse/useListingSession';
import { otherPaneSession } from './otherPane';

const session = (name: string) => ({ name }) as unknown as ListingSession;
const ready = (s: ListingSession): SessionState => ({ status: 'ready', session: s });

function snapshot(...pairs: number[][]): SessionSnapshot {
	return {
		tabs: [],
		active: 1,
		pairs: pairs.map(
			(panes, index) =>
				({
					id: index + 1,
					panes,
					layout: 'sideBySide',
					sizes: [],
					origin: { kind: 'joined' },
				}) as unknown as Pair,
		),
	} as unknown as SessionSnapshot;
}

describe('otherPaneSession', () => {
	const a = session('a');
	const b = session('b');
	const c = session('c');
	const states: Record<number, SessionState> = { 1: ready(a), 2: ready(b), 3: ready(c) };
	const stateFor = (tab: number) => states[tab];

	it('finds the other half of a pair, from either side', () => {
		expect(otherPaneSession(snapshot([1, 2]), a, stateFor)).toBe(b);
		expect(otherPaneSession(snapshot([1, 2]), b, stateFor)).toBe(a);
	});

	it('takes the next pane in order, wrapping, when a pair has more than two', () => {
		const three = snapshot([1, 2, 3]);
		expect(otherPaneSession(three, a, stateFor)).toBe(b);
		expect(otherPaneSession(three, c, stateFor)).toBe(a);
	});

	it('finds the pair that holds the pane among several', () => {
		expect(otherPaneSession(snapshot([4, 5], [1, 2]), b, stateFor)).toBe(a);
	});

	it('is null for a pane that is not paired, and without a session', () => {
		expect(otherPaneSession(snapshot([2, 3]), a, stateFor)).toBeNull();
		expect(otherPaneSession(snapshot(), a, stateFor)).toBeNull();
		expect(otherPaneSession(null, a, stateFor)).toBeNull();
	});

	it('is null while the other pane’s listing is opening or failed', () => {
		const opening: Record<number, SessionState> = { 1: ready(a), 2: { status: 'opening' } };
		expect(otherPaneSession(snapshot([1, 2]), a, (tab) => opening[tab])).toBeNull();
		const missing: Record<number, SessionState> = { 1: ready(a) };
		expect(otherPaneSession(snapshot([1, 2]), a, (tab) => missing[tab])).toBeNull();
	});
});
