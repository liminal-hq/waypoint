// The name field that replaces a row's name while it is renamed in place, in the list and the grid
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Dialog } from '@liminal-hq/waypoint-chrome/Dialog/Dialog';
import { DialogActions, DialogButton } from '@liminal-hq/waypoint-chrome/Dialog/DialogActions';
import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import {
	useEffect,
	useId,
	useLayoutEffect,
	useRef,
	useState,
	type KeyboardEvent,
	type MouseEvent,
} from 'react';
import { t, tf } from '../i18n/messages';
import { isFolder } from '../nav/useOpenEntry';
import type { FileCommands } from '../ops/fileCommands';
import styles from './InlineRename.module.css';
import { checkName, hostNameRule, type NameProblem } from './nameRules';
import {
	extensionChange,
	initialSelection,
	withExtension,
	type ExtensionChange,
} from './renameModel';
import type { ListingSession } from './useListingSession';

interface InlineRenameProps {
	entry: Entry;
	session: ListingSession;
	commands: FileCommands;
	variant: 'list' | 'grid';
	/** The field has ended, by a rename or by cancelling: the view puts focus back on its list. */
	onFinish: () => void;
}

/** The sentence for a problem with the name as typed. */
export function problemText(problem: NameProblem): string {
	switch (problem.kind) {
		case 'tooLong':
			return tf('rename.error.tooLong', { limit: problem.limit });
		case 'forbidden': {
			const code = problem.character.codePointAt(0) ?? 0;
			const shown =
				code < 0x20 || (code >= 0x7f && code <= 0x9f)
					? `U+${code.toString(16).toUpperCase().padStart(4, '0')}`
					: `“${problem.character}”`;
			return tf('rename.error.forbidden', { character: shown });
		}
		default:
			return t(`rename.error.${problem.kind}`);
	}
}

/** A mouse event inside the field is the field's, not the row's: it must not select, open or open a menu. */
const keepToField = (event: MouseEvent) => event.stopPropagation();

/**
 * Renames in place. The stem of a file starts selected (the whole name of a folder), Enter
 * renames, and Escape or a click elsewhere cancels. What is wrong with the name shows under the
 * field as it is typed, and a name Rust refuses (one that is taken) keeps the field open with the
 * reason. Changing a file's extension asks first. The row keeps the focus ring: when the field
 * ends, `onFinish` returns focus to the list.
 */
export function InlineRename({ entry, session, commands, variant, onFinish }: InlineRenameProps) {
	const folder = isFolder(entry);
	const rule = hostNameRule();
	const [value, setValue] = useState(entry.name);
	const [message, setMessage] = useState<string | null>(null);
	const [pending, setPending] = useState(false);
	const [guard, setGuard] = useState<{ typed: string; change: ExtensionChange } | null>(null);
	const input = useRef<HTMLInputElement | null>(null);
	const alive = useRef(true);
	const busy = useRef(false);
	const hintId = useId();
	const problemId = useId();

	useLayoutEffect(() => {
		const field = input.current;
		if (!field) return;
		field.focus();
		const [from, to] = initialSelection(entry.name, folder);
		field.setSelectionRange(from, to);
	}, [entry.name, folder]);

	useEffect(() => {
		alive.current = true;
		return () => {
			alive.current = false;
			// Unmounted mid-rename (the listing moved the row): do not let the row's new place open a second field.
			if (busy.current) session.store.getState().endRename();
		};
	}, []);

	const finish = () => {
		session.store.getState().endRename();
		onFinish();
	};

	const refocus = () => {
		input.current?.focus();
	};

	const submit = async (name: string) => {
		if (name === entry.name) return finish();
		busy.current = true;
		setPending(true);
		const outcome = await commands.renameEntry(session, entry, name);
		busy.current = false;
		// A rename can move the row (the listing re-sorts it), which unmounts this field before the
		// answer arrives; the rename is over all the same, so the store must be told.
		if (outcome.ok) return finish();
		if (!alive.current) return;
		setPending(false);
		setMessage(outcome.message);
		refocus();
	};

	const commit = () => {
		if (busy.current) return;
		const problem = checkName(value, rule);
		if (problem) {
			setMessage(problemText(problem));
			return;
		}
		if (value === entry.name) return finish();
		const change = extensionChange(entry.name, value, folder);
		if (change) {
			setGuard({ typed: value, change });
			return;
		}
		void submit(value);
	};

	const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
		if (event.nativeEvent.isComposing) return;
		if (event.key === 'Enter') {
			event.preventDefault();
			event.stopPropagation();
			commit();
		} else if (event.key === 'Escape') {
			event.preventDefault();
			event.stopPropagation();
			finish();
		}
	};

	const closeGuard = () => {
		setGuard(null);
		// Back to the field with what was typed, so the person can change their mind.
		queueMicrotask(refocus);
	};

	return (
		<span
			className={styles.wrap}
			data-variant={variant}
			onMouseDown={keepToField}
			onClick={keepToField}
			onDoubleClick={keepToField}
			onContextMenu={keepToField}
			onAuxClick={keepToField}
		>
			<input
				ref={input}
				className={styles.field}
				type="text"
				value={value}
				spellCheck={false}
				autoComplete="off"
				aria-label={tf('rename.field.label', { name: entry.name })}
				aria-describedby={`${hintId} ${problemId}`}
				aria-invalid={message !== null}
				aria-busy={pending}
				readOnly={pending}
				onChange={(event) => {
					const next = event.target.value;
					setValue(next);
					const problem = checkName(next, rule);
					setMessage(problem ? problemText(problem) : null);
				}}
				onKeyDown={onKeyDown}
				onBlur={(event) => {
					// A question or a rename in flight owns focus for now, and a window that loses focus
					// keeps its field; only a click or a Tab to somewhere else cancels.
					if (guard || busy.current) return;
					if (document.activeElement === event.currentTarget) return;
					finish();
				}}
			/>
			<span id={hintId} className={styles.srOnly}>
				{t('rename.hint')}
			</span>
			<span id={problemId} className={styles.problem} role="status" aria-live="polite">
				{message}
			</span>
			{guard && (
				<Dialog
					open
					title={t('rename.extension.title')}
					description={
						guard.change.to === ''
							? tf('rename.extension.remove', { from: guard.change.from })
							: tf('rename.extension.change', { from: guard.change.from, to: guard.change.to })
					}
					size="small"
					onClose={closeGuard}
					footer={
						<DialogActions>
							<DialogButton
								variant="secondary"
								onClick={() => {
									const { typed } = guard;
									setGuard(null);
									void submit(typed);
								}}
							>
								{guard.change.to === ''
									? tf('rename.extension.drop', { from: guard.change.from })
									: tf('rename.extension.use', { to: guard.change.to })}
							</DialogButton>
							<DialogButton
								variant="primary"
								onClick={() => {
									const kept = withExtension(guard.typed, guard.change.from);
									setGuard(null);
									setValue(kept);
									void submit(kept);
								}}
							>
								{tf('rename.extension.keep', { from: guard.change.from })}
							</DialogButton>
						</DialogActions>
					}
				/>
			)}
		</span>
	);
}
