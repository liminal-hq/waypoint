// Pure helpers for following a listing's patches: where a view position lands after a batch of edits
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { PatchOp } from '@liminal-hq/waypoint-protocol/generated/PatchOp';

export interface MappedPosition {
	/** Where the position ends up. For a removed entry, where its successor now sits. */
	position: number;
	/** Whether the entry at the original position was removed by the patch. */
	removed: boolean;
}

/**
 * Follows one view position through a patch. Ops apply in order and each one's positions are in the
 * coordinates left by the ones before it (removals from the highest position down, then
 * insertions ascending, then updates; see `diff()` in `fakeVfsClient.ts`). A `reset` carries no
 * position information, so the position is returned unchanged for the caller to clamp.
 */
export function mapPosition(position: number, ops: readonly PatchOp[]): MappedPosition {
	let current = position;
	let removed = false;
	for (const op of ops) {
		switch (op.kind) {
			case 'remove':
				if (current >= op.at + op.count) {
					current -= op.count;
				} else if (current >= op.at) {
					removed = true;
					current = op.at;
				}
				break;
			case 'insert':
				// A removed entry's successor already sits at `current`; an insert at or before
				// it still pushes the successor down.
				if (current >= op.at) current += op.count;
				break;
			case 'update':
			case 'reset':
				break;
		}
	}
	return { position: current, removed };
}

/** Whether a patch replaces the whole view instead of describing edits to it. */
export function isReset(ops: readonly PatchOp[]): boolean {
	return ops.some((op) => op.kind === 'reset');
}
