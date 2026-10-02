// Builds command facts for tests: a window in a known state, from the few things that decide availability
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { JournalEntrySummary } from '@liminal-hq/waypoint-protocol/generated/JournalEntrySummary';
import type { SortSpec } from '@liminal-hq/waypoint-protocol/generated/SortSpec';
import { emptyFacts, type CommandFacts } from '../commands/commandEnv';
import { commandStates } from '../ops/fileCommands';

export interface WindowState {
	/** The window has a queue (the Main window does). */
	queue?: boolean;
	listing?: boolean;
	readOnly?: boolean;
	trash?: boolean;
	selected?: number;
	focused?: boolean;
	clipboardItems?: number;
	paired?: boolean;
	otherPaneWritable?: boolean;
	undo?: JournalEntrySummary | null;
	redo?: JournalEntrySummary | null;
}

export const SORT: SortSpec = { key: 'name', descending: false, directoriesFirst: true };

export const entry = (id: number, label: string, extra: Partial<JournalEntrySummary> = {}) =>
	({
		id,
		label,
		atMs: new Date(2026, 9, 1, 14, 30).getTime(),
		undoable: true,
		redoable: false,
		partlyUndone: false,
		...extra,
	}) as JournalEntrySummary;

/** A folder window with a queue, one tab open and nothing selected, changed by `state` and `extra`. */
export function factsFor(state: WindowState = {}, extra: Partial<CommandFacts> = {}): CommandFacts {
	const listing = state.listing ?? true;
	const undo = state.undo ?? null;
	const redo = state.redo ?? null;
	const selected = state.selected ?? 0;
	return {
		...emptyFacts(),
		file: commandStates({
			queue: state.queue ?? true,
			listing,
			readOnly: state.readOnly ?? false,
			selected,
			focused: state.focused ?? selected > 0,
			undo,
			redo,
			trash: state.trash ?? false,
			clipboardItems: state.clipboardItems ?? 0,
			paired: state.paired ?? false,
			otherPaneWritable: state.otherPaneWritable ?? false,
		}),
		selected,
		listing,
		trash: state.trash ?? false,
		batchRename: selected > 0 && !(state.readOnly ?? false),
		sort: listing ? SORT : null,
		undoLabel: undo?.label ?? null,
		redoLabel: redo?.label ?? null,
		history: [undo, redo].flatMap((item) => (item ? [item] : [])),
		undoHead: undo?.id ?? null,
		redoHead: redo?.id ?? null,
		tab: true,
		tabCount: 1,
		paired: state.paired ?? false,
		...extra,
	};
}
