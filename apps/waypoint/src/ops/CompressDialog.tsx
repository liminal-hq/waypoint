// The compress dialog: a name and a format for the new archive, with the extension kept in step
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Dialog } from '@liminal-hq/waypoint-chrome/Dialog/Dialog';
import { DialogActions, DialogButton } from '@liminal-hq/waypoint-chrome/Dialog/DialogActions';
import type { ArchiveFormat } from '@liminal-hq/waypoint-protocol/generated/ArchiveFormat';
import { useId, useState, type FormEvent } from 'react';
import { COMPRESS_FORMATS, stripArchiveEnd, withExtension } from '../archives/archiveNames';
import { t, type MessageId } from '../i18n/messages';
import type { CompressChoice, CompressOptions } from './compressStore';
import styles from './CompressDialog.module.css';

interface CompressDialogProps {
	options: CompressOptions;
	onConfirm: (choice: CompressChoice) => void;
	onCancel: () => void;
}

/** What is wrong with a name typed for an archive, in words, or `null` when it is fine. */
export function nameProblem(name: string): MessageId | null {
	if (name.trim() === '') return 'compress.error.empty';
	if (/[\\/]/.test(name)) return 'compress.error.invalid';
	return null;
}

/**
 * Names the archive and picks its format. Focus starts on the name, selected, so typing replaces it;
 * Enter compresses. Changing the format changes the end of the name, so the name and the format
 * never disagree. A name with a slash or nothing in it is refused where it is typed, in words
 * that are read out.
 */
export function CompressDialog({ options, onConfirm, onCancel }: CompressDialogProps) {
	const [format, setFormat] = useState<ArchiveFormat>(options.format ?? 'zip');
	const [name, setName] = useState(stripArchiveEnd(options.name));
	const nameId = useId();
	const formatId = useId();
	const problemId = useId();
	const problem = nameProblem(name);
	const [touched, setTouched] = useState(false);

	const submit = (event?: FormEvent) => {
		event?.preventDefault();
		if (problem) {
			setTouched(true);
			return;
		}
		onConfirm({ name: withExtension(name.trim(), format), format });
	};

	return (
		<Dialog
			open
			size="small"
			title={t('compress.title')}
			description={t('compress.description')}
			initialFocus={`#${CSS.escape(nameId)}`}
			onClose={onCancel}
			footer={
				<DialogActions>
					<DialogButton variant="secondary" onClick={onCancel}>
						{t('compress.cancel')}
					</DialogButton>
					<DialogButton variant="primary" onClick={() => submit()}>
						{t('compress.confirm')}
					</DialogButton>
				</DialogActions>
			}
		>
			<form className={styles.form} onSubmit={submit}>
				<div className={styles.field}>
					<label className={styles.label} htmlFor={nameId}>
						{t('compress.name')}
					</label>
					<input
						id={nameId}
						className={styles.input}
						value={name}
						autoComplete="off"
						spellCheck={false}
						aria-invalid={touched && problem ? true : undefined}
						aria-describedby={problemId}
						onFocus={(event) => event.currentTarget.select()}
						onChange={(event) => {
							setName(event.target.value);
							setTouched(true);
						}}
					/>
					<p id={problemId} className={styles.problem} role="alert">
						{touched && problem ? t(problem) : ''}
					</p>
				</div>
				<div className={styles.field}>
					<label className={styles.label} htmlFor={formatId}>
						{t('compress.format')}
					</label>
					<select
						id={formatId}
						className={styles.select}
						value={format}
						onChange={(event) => setFormat(event.target.value as ArchiveFormat)}
					>
						{COMPRESS_FORMATS.map((choice) => (
							<option key={choice} value={choice}>
								{t(`compress.format.${choice}` as MessageId)}
							</option>
						))}
					</select>
				</div>
				{/* Enter in the name compresses. */}
				<button type="submit" hidden tabIndex={-1} aria-hidden="true" />
			</form>
		</Dialog>
	);
}
