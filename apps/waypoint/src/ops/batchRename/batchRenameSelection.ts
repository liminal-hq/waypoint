// What a batch rename acts on: the active pane's selection, where its listing can be written to
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { selectedCount } from '../../browse/selection';
import type { ListingSession } from '../../browse/useListingSession';
import { selectionSources } from '../fileCommands';
import type { BatchRenameSelection } from './batchRenameApi';

/** The selection to batch rename, or `null` with nothing selected or where nothing can be written (the Trash, an archive). */
export function batchRenameSelection(session: ListingSession | null): BatchRenameSelection | null {
	if (!session || session.model.readOnly) return null;
	const { selection } = session.store.getState();
	const count = selectedCount(selection, session.model.count);
	if (count === 0) return null;
	return { sources: selectionSources(session.model.handle, selection), count };
}
