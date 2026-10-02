// Owns the command palette for a Main window: opens it from the key and the menu, then runs what is chosen
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { showNotice } from '../app/notices';
import { useHourCycle } from '../browse/TimeFormatContext';
import { t } from '../i18n/messages';
import { useConfirm } from '../ops/ConfirmHost';
import { useCommandBridge, useCommands } from './commandBridge';
import { CommandPalette } from './CommandPalette';
import {
	applyHistory,
	confirmSpec,
	historyRows,
	needsConfirmation,
	sameChain,
	type HistoryRow,
} from './historyCommands';
import type { PaletteTarget } from './paletteModel';
import type { CommandId } from './registry';

/** The commands remembered as recent; the palette shows the newest few. */
const REMEMBERED = 10;

/** Ctrl+Shift+P, with Cmd for a keyboard that has one. It is not a text-editing key, so it works in a field too. */
export function isPaletteKey(event: {
	key: string;
	ctrlKey: boolean;
	metaKey: boolean;
	altKey: boolean;
	shiftKey: boolean;
	isComposing?: boolean;
}): boolean {
	return (
		event.key.toLowerCase() === 'p' &&
		(event.ctrlKey || event.metaKey) &&
		event.shiftKey &&
		!event.altKey &&
		!event.isComposing
	);
}

interface Opened {
	query: string;
	recents: CommandId[];
}

/**
 * Renders the palette while it is open, and the confirmation an older history entry asks. A
 * chosen command runs through the registry (`useCommands().run`), the same call the menu makes;
 * a history row runs its chain through `FileCommands.stepHistory` and says how it went. The
 * commands run last are kept for this window only.
 */
export function CommandPaletteHost() {
	const bridge = useCommandBridge();
	const { commands, facts, run } = useCommands();
	const hourCycle = useHourCycle();
	const { confirm, dialog } = useConfirm();
	const [opened, setOpened] = useState<Opened | null>(null);
	const recents = useRef<CommandId[]>([]);
	/** A history chain is deciding or running; a second one (or an Undo or Redo) would interleave with it. */
	const chainInFlight = useRef(false);
	const isOpen = useRef(false);
	isOpen.current = opened !== null;

	const open = useCallback((query = '') => {
		setOpened({ query, recents: [...recents.current] });
	}, []);
	const close = useCallback(() => setOpened(null), []);

	useEffect(() => {
		bridge.patchActions({ openPalette: open });
	}, [bridge, open]);

	useEffect(() => {
		const onKeyDown = (event: KeyboardEvent) => {
			if (event.defaultPrevented || !isPaletteKey(event)) return;
			event.preventDefault();
			if (isOpen.current) return setOpened(null);
			// Another dialog has the person's attention; the palette would stack over it.
			if (document.querySelector('dialog[open]')) return;
			open();
		};
		window.addEventListener('keydown', onKeyDown);
		return () => window.removeEventListener('keydown', onKeyDown);
	}, [open]);

	const rows: HistoryRow[] = useMemo(
		() => historyRows(facts.history, { undoHead: facts.undoHead, redoHead: facts.redoHead }),
		[facts.history, facts.undoHead, facts.redoHead],
	);

	const execute = useCallback(
		async (target: PaletteTarget) => {
			if (target.kind === 'command') {
				if (chainInFlight.current && (target.id === 'undo' || target.id === 'redo')) {
					showNotice(t('history.busy'));
					return;
				}
				if (run(target.id)) {
					recents.current = [target.id, ...recents.current.filter((id) => id !== target.id)].slice(
						0,
						REMEMBERED,
					);
				}
				return;
			}
			const files = bridge.store.getState().actions.files;
			if (!files) return;
			if (chainInFlight.current) {
				showNotice(t('history.busy'));
				return;
			}
			chainInFlight.current = true;
			try {
				if (needsConfirmation(target.row) && !(await confirm(confirmSpec(target.row)))) return;
				// The row was built before the question: a job may have finished since, so the chain is
				// read again from the history as it is now and must be what was confirmed.
				const { history, undoHead, redoHead } = bridge.store.getState().facts;
				const current = historyRows(history, { undoHead, redoHead }).find(
					(candidate) => candidate.key === target.row.key,
				);
				if (!sameChain(current, target.row)) {
					showNotice(t('history.changed'));
					return;
				}
				const report = await applyHistory(current!, {
					step: (kind, entry) => files.stepHistory(kind, entry),
				});
				showNotice(report.text);
			} finally {
				chainInFlight.current = false;
			}
		},
		[bridge, confirm, run],
	);

	const choose = useCallback(
		(target: PaletteTarget) => {
			// Close first so focus is back in the window before a command moves it (Rename focuses its field).
			setOpened(null);
			setTimeout(() => void execute(target), 0);
		},
		[execute],
	);

	return (
		<>
			{opened && (
				<CommandPalette
					commands={commands}
					history={rows}
					recents={opened.recents}
					initialQuery={opened.query}
					hourCycle={hourCycle}
					onChoose={choose}
					onClose={close}
				/>
			)}
			{dialog}
		</>
	);
}
