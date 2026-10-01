// Verifies which selection a batch rename acts on: none where nothing is selected or the listing is read only
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import type { ListingSession } from '../../browse/useListingSession';
import { batchRenameSelection } from './batchRenameSelection';

const session = (readOnly: boolean, selection: unknown, count = 5) =>
	({
		model: { readOnly, count, handle: 7 },
		store: { getState: () => ({ selection }) },
	}) as unknown as ListingSession;

describe('batchRenameSelection', () => {
	it('is the selection as a handle and a range, with its count', () => {
		const some = { kind: 'some', ids: new Set([1, 2]) };
		expect(batchRenameSelection(session(false, some))).toEqual({
			sources: { kind: 'selection', handle: 7, spec: { kind: 'some', ids: [1, 2] } },
			count: 2,
		});
	});

	it('is nothing with no selection, no pane, or a read-only listing', () => {
		const some = { kind: 'some', ids: new Set([1, 2]) };
		expect(batchRenameSelection(session(false, { kind: 'some', ids: new Set() }))).toBeNull();
		expect(batchRenameSelection(null)).toBeNull();
		expect(batchRenameSelection(session(true, some))).toBeNull();
	});
});
