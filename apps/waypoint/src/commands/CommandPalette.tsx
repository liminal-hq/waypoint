// The command palette: a modal list of the window's commands and undo history, filtered as you type
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { prefersReducedMotion } from '../theme/motion';
import {
	useEffect,
	useId,
	useLayoutEffect,
	useMemo,
	useRef,
	useState,
	type KeyboardEvent as ReactKeyboardEvent,
	type MouseEvent as ReactMouseEvent,
} from 'react';
import { t, tf } from '../i18n/messages';
import type { HourCycle } from '../services/timeFormatClient';
import styles from './CommandPalette.module.css';
import { splitByRanges } from './fuzzy';
import type { HistoryRow } from './historyCommands';
import {
	countText,
	moveActive,
	paletteRows,
	type PaletteRow,
	type PaletteTarget,
} from './paletteModel';
import type { CommandId, CommandView } from './registry';

/** How far PageUp and PageDown move the active row. */
const PAGE = 8;

export interface CommandPaletteProps {
	/** The registry resolved for the window now. */
	commands: readonly CommandView[];
	/** The history's rows, offered once something is typed. */
	history: readonly HistoryRow[];
	/** The commands run last, newest first. */
	recents: readonly CommandId[];
	/** What is typed when it opens ("undo" from the history menu's footer). */
	initialQuery?: string;
	hourCycle?: HourCycle;
	/** A row that can run was chosen. The host closes the palette and runs it. */
	onChoose: (target: PaletteTarget) => void;
	onClose: () => void;
}

/**
 * A native modal `<dialog>` holding a combobox (the input) and the listbox it controls, with
 * focus staying in the input while the arrow keys move the active option
 * (`aria-activedescendant`). The page behind is inert while it is open, Esc, a click outside and
 * the window losing focus close it, and focus goes back to where it was.
 *
 * A row that cannot run stays in the list, says why, and answers Enter and a click by announcing
 * that reason. The live region says how many rows the query left.
 */
export function CommandPalette({
	commands,
	history,
	recents,
	initialQuery = '',
	hourCycle,
	onChoose,
	onClose,
}: CommandPaletteProps) {
	const dialogRef = useRef<HTMLDialogElement>(null);
	const inputRef = useRef<HTMLInputElement>(null);
	const listRef = useRef<HTMLUListElement>(null);
	const listId = useId();
	const [query, setQuery] = useState(initialQuery);
	const [active, setActive] = useState(0);
	const [hover, setHover] = useState<number | null>(null);
	// A reason read out for a row that cannot run, until the query or the active row changes.
	const [notice, setNotice] = useState<string | null>(null);
	const [native, setNative] = useState(true);
	const reducedMotion = useMemo(prefersReducedMotion, []);

	const rows = useMemo(
		() => paletteRows({ commands, history, query, recents, hourCycle }),
		[commands, history, query, recents, hourCycle],
	);
	const current = rows.length === 0 ? -1 : Math.min(active, rows.length - 1);

	const live = useRef({ onClose });
	live.current = { onClose };

	// Open as a modal and focus the input; on close hand focus back to where it was.
	useLayoutEffect(() => {
		const dialog = dialogRef.current;
		if (!dialog) return;
		const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
		if (typeof dialog.showModal === 'function') {
			try {
				if (!dialog.open) dialog.showModal();
			} catch {
				dialog.setAttribute('open', '');
				setNative(false);
			}
		} else {
			dialog.setAttribute('open', '');
			setNative(false);
		}
		const input = inputRef.current;
		input?.focus();
		input?.setSelectionRange(input.value.length, input.value.length);

		// The platform's own Esc would close the dialog behind React's back; closing is the host's.
		const onCancel = (event: Event) => {
			event.preventDefault();
			live.current.onClose();
		};
		// Another application taking focus is a reason to put the palette away.
		// A blur on the window itself: a field's own blur does not bubble to it.
		const onBlur = () => live.current.onClose();
		dialog.addEventListener('cancel', onCancel);
		window.addEventListener('blur', onBlur);
		return () => {
			dialog.removeEventListener('cancel', onCancel);
			window.removeEventListener('blur', onBlur);
			if (dialog.open && typeof dialog.close === 'function') dialog.close();
			if (opener?.isConnected) opener.focus();
		};
	}, []);

	useEffect(() => {
		if (current < 0) return;
		const option = listRef.current?.children[current];
		if (option instanceof HTMLElement && typeof option.scrollIntoView === 'function') {
			option.scrollIntoView({ block: 'nearest' });
		}
	}, [current, rows]);

	const choose = (row: PaletteRow | undefined) => {
		if (!row) return;
		if (!row.enabled) {
			setNotice(tf('palette.cannotRun', { name: row.label, reason: row.reason ?? '' }));
			return;
		}
		onChoose(row.target);
	};

	const move = (by: number, wrap = false) => {
		setNotice(null);
		setHover(null);
		setActive(moveActive(current, by, rows.length, wrap));
	};

	const onKeyDown = (event: ReactKeyboardEvent<HTMLInputElement>) => {
		if (event.nativeEvent.isComposing) return;
		switch (event.key) {
			case 'ArrowDown':
				return stop(event, () => move(1, true));
			case 'ArrowUp':
				return stop(event, () => move(-1, true));
			case 'PageDown':
				return stop(event, () => move(PAGE));
			case 'PageUp':
				return stop(event, () => move(-PAGE));
			case 'Home':
				return stop(event, () => move(-Infinity));
			case 'End':
				return stop(event, () => move(Infinity));
			case 'Enter':
				return stop(event, () => choose(rows[current]));
			case 'Escape':
				return stop(event, () => onClose());
			case 'Tab':
				// The palette is one control; Tab has nowhere else to go inside it.
				return event.preventDefault();
		}
	};

	const onBackdrop = (event: ReactMouseEvent<HTMLDialogElement>) => {
		if (event.target === event.currentTarget) onClose();
	};

	const optionId = (index: number) => `${listId}-${index}`;
	const status = notice ?? countText(rows.length);

	return (
		<dialog
			ref={dialogRef}
			className={[
				styles.palette,
				native ? '' : styles.fallback,
				reducedMotion ? styles.reducedMotion : '',
			]
				.filter(Boolean)
				.join(' ')}
			aria-label={t('palette.title')}
			data-reduced-motion={reducedMotion ? '' : undefined}
			onClick={onBackdrop}
		>
			<div className={styles.surface}>
				<input
					ref={inputRef}
					className={styles.input}
					type="text"
					role="combobox"
					aria-label={t('palette.placeholder')}
					aria-expanded={rows.length > 0}
					aria-controls={listId}
					aria-autocomplete="list"
					aria-activedescendant={current >= 0 ? optionId(current) : undefined}
					placeholder={t('palette.placeholder')}
					value={query}
					autoComplete="off"
					autoCorrect="off"
					spellCheck={false}
					onChange={(event) => {
						setQuery(event.target.value);
						setActive(0);
						setHover(null);
						setNotice(null);
					}}
					onKeyDown={onKeyDown}
				/>
				<ul
					ref={listRef}
					id={listId}
					className={styles.list}
					role="listbox"
					aria-label={t('palette.list.label')}
					hidden={rows.length === 0}
				>
					{rows.map((row, index) => (
						<PaletteOption
							key={row.key}
							row={row}
							id={optionId(index)}
							active={index === current}
							hovered={index === hover}
							onHover={() => {
								if (hover !== index) setHover(index);
								if (active !== index) {
									setActive(index);
									setNotice(null);
								}
							}}
							onChoose={() => choose(row)}
						/>
					))}
				</ul>
				{rows.length === 0 && <p className={styles.empty}>{t('palette.count.none')}</p>}
				<div className={styles.srOnly} role="status" aria-live="polite">
					{status}
				</div>
			</div>
		</dialog>
	);
}

function stop(event: ReactKeyboardEvent, then: () => void) {
	event.preventDefault();
	then();
}

interface PaletteOptionProps {
	row: PaletteRow;
	id: string;
	active: boolean;
	hovered: boolean;
	onHover: () => void;
	onChoose: () => void;
}

function PaletteOption({ row, id, active, hovered, onHover, onChoose }: PaletteOptionProps) {
	const Icon = row.icon;
	return (
		<li
			id={id}
			role="option"
			aria-selected={active}
			aria-disabled={row.enabled ? undefined : true}
			aria-label={row.name}
			data-hover={hovered && !active ? '' : undefined}
			className={styles.option}
			// The pointer never takes focus from the input: typing keeps working after a click.
			onMouseDown={(event) => event.preventDefault()}
			onMouseMove={onHover}
			onClick={onChoose}
		>
			<span className={styles.icon} aria-hidden="true">
				{Icon ? <Icon /> : null}
			</span>
			<span className={styles.text}>
				<span className={styles.label}>
					{splitByRanges(row.label, row.ranges).map((part, index) =>
						part.match ? (
							<b key={index} className={styles.match}>
								{part.text}
							</b>
						) : (
							<span key={index}>{part.text}</span>
						),
					)}
				</span>
				<span className={styles.hint}>{row.hint}</span>
				{row.reason && <span className={styles.reason}>{row.reason}</span>}
			</span>
			<span className={styles.side}>
				{row.checked && (
					<span className={styles.check} aria-hidden="true">
						{'✓'}
					</span>
				)}
				{row.detail && <span title={row.detailTitle}>{row.detail}</span>}
				{row.shortcut && <kbd className={styles.shortcut}>{row.shortcut}</kbd>}
			</span>
		</li>
	);
}
