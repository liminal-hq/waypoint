// The conflict resolver: one dialog for the names a job found already taken, answered row by row or for all at once
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ConfirmDialog } from '@liminal-hq/waypoint-chrome/Dialog/ConfirmDialog';
import { Dialog } from '@liminal-hq/waypoint-chrome/Dialog/Dialog';
import { DialogActions, DialogButton } from '@liminal-hq/waypoint-chrome/Dialog/DialogActions';
import type { Conflict } from '@liminal-hq/waypoint-protocol/generated/Conflict';
import type { ConflictPolicy } from '@liminal-hq/waypoint-protocol/generated/ConflictPolicy';
import type { JobSnapshot } from '@liminal-hq/waypoint-protocol/generated/JobSnapshot';
import type { Resolution } from '@liminal-hq/waypoint-protocol/generated/Resolution';
import { useId, useMemo, useState } from 'react';
import { formatModified, formatSize } from '../browse/format';
import { useHourCycle } from '../browse/TimeFormatContext';
import { t, tf, type MessageId } from '../i18n/messages';
import {
	NO_ANSWERS,
	ROW_CAP,
	bulkChoices,
	buildAnswer,
	coverage,
	dateHint,
	effectivePolicy,
	existingIsFolder,
	isDestructive,
	legalChoices,
	type Answers,
	type Coverage,
} from './conflictModel';
import styles from './ConflictDialog.module.css';
import { pluralText, policyLabel } from './resolveText';

/** What the person answered, ready for `OpsClient.resolve`. */
export interface ConflictAnswer {
	decisions: Resolution[];
	applyToAll?: ConflictPolicy;
	/** How many clashes each choice settles, for saying what happens next. */
	counts: Coverage['byPolicy'];
}

export interface ConflictDialogProps {
	/** The job that is waiting: its destination and kind word the dialog. */
	job: JobSnapshot;
	conflicts: readonly Conflict[];
	onContinue(answer: ConflictAnswer): void;
	/** Stops the whole job. Nothing has been written when the job waits on a batch. */
	onCancelJob(): void;
	/** Closes the dialog and leaves the job waiting; "Resolve…" on its row opens it again. */
	onLater(): void;
}

function entryText(
	isFolder: boolean,
	size: number | null,
	modifiedMs: number | null,
	hourCycle: ReturnType<typeof useHourCycle>,
): string {
	const parts = [t(isFolder ? 'ops.conflict.kind.folder' : 'ops.conflict.kind.file')];
	if (size !== null) parts.push(formatSize(size));
	else if (!isFolder) parts.push(t('ops.conflict.sizeUnknown'));
	parts.push(
		modifiedMs !== null
			? formatModified(modifiedMs, undefined, hourCycle)
			: t('ops.conflict.modifiedUnknown'),
	);
	return parts.join(' · ');
}

/** What a choice does, said in words so the danger of a replace is never only a colour. */
function choiceNote(policy: ConflictPolicy, replacesFolder: boolean, restoring: boolean): string {
	switch (policy) {
		case 'replace':
			return t(
				replacesFolder ? 'ops.conflict.note.replace.folder' : 'ops.conflict.note.replace.file',
			);
		case 'keepBoth':
			return t(restoring ? 'ops.conflict.note.keepBoth.restore' : 'ops.conflict.note.keepBoth');
		default:
			return t(`ops.conflict.note.${policy}` as MessageId);
	}
}

/**
 * Opens with nothing chosen: "Continue" stays off until every clash has an answer of its own or
 * the "Apply to all remaining" choice reaches it, and the first control has focus, never a
 * destructive one. Escape and "Cancel the operation" stop the job (after asking, once anything
 * has been answered); a click outside does nothing. A choice is offered for a clash only where the
 * engine can carry it out.
 */
export function ConflictDialog({
	job,
	conflicts,
	onContinue,
	onCancelJob,
	onLater,
}: ConflictDialogProps) {
	const hourCycle = useHourCycle();
	const baseId = useId();
	const [answers, setAnswers] = useState<Answers>(NO_ANSWERS);
	const [showAll, setShowAll] = useState(false);
	const [confirming, setConfirming] = useState(false);

	const restoring = job.kind.kind === 'restore';
	const count = conflicts.length;
	const stats = useMemo(() => coverage(conflicts, answers), [conflicts, answers]);
	const bulkOptions = useMemo(() => bulkChoices(conflicts, answers), [conflicts, answers]);
	const complete = stats.answered === stats.total;
	const anyAnswer = answers.rows.size > 0 || answers.bulk !== null;
	const visible = showAll ? conflicts : conflicts.slice(0, ROW_CAP);
	const hidden = count - visible.length;

	const destination = job.destination?.display ?? t('ops.conflict.destination.unknown');
	const title = restoring
		? pluralText('ops.conflict.title.restore', count)
		: pluralText('ops.conflict.title', count, { destination });

	const setRow = (conflict: Conflict, value: string) =>
		setAnswers((now) => {
			const rows = new Map(now.rows);
			if (value === '') rows.delete(conflict.source.uri);
			else rows.set(conflict.source.uri, value as ConflictPolicy);
			return { ...now, rows };
		});
	const setBulk = (value: string) =>
		setAnswers((now) => ({
			...now,
			bulk: value === '' ? null : (value as ConflictPolicy),
			later: value === '' ? false : now.later,
		}));

	const bulkReplacesFolder =
		answers.bulk === 'replace' &&
		conflicts.some(
			(c) =>
				!answers.rows.has(c.source.uri) && existingIsFolder(c.kind) && effectivePolicy(c, answers),
		);

	const requestCancel = () => (anyAnswer ? setConfirming(true) : onCancelJob());
	const submit = () => {
		const answer = buildAnswer(conflicts, answers);
		if (!answer) return;
		onContinue({ ...answer, counts: stats.byPolicy });
	};

	const bulkId = `${baseId}-bulk`;
	const bulkHintId = `${baseId}-bulk-hint`;
	const laterId = `${baseId}-later`;
	const laterHintId = `${baseId}-later-hint`;

	return (
		<Dialog
			open
			size="large"
			title={title}
			description={t(restoring ? 'ops.conflict.description.restore' : 'ops.conflict.description')}
			onClose={(reason) => {
				// A click outside must not cancel a job by accident; Escape is the keyboard way out.
				if (reason !== 'backdrop') requestCancel();
			}}
			footer={
				<div className={styles.footer}>
					<p className={styles.progress} role="status" aria-live="polite">
						{tf('ops.conflict.progress', { answered: stats.answered, total: stats.total })}
					</p>
					<DialogActions>
						<DialogButton variant="secondary" onClick={onLater}>
							{t('ops.conflict.later')}
						</DialogButton>
						<DialogButton variant="secondary" onClick={requestCancel}>
							{t('ops.conflict.cancel')}
						</DialogButton>
						<DialogButton variant="primary" disabled={!complete} onClick={submit}>
							{t('ops.conflict.continue')}
						</DialogButton>
					</DialogActions>
				</div>
			}
		>
			<div className={styles.layout}>
				<fieldset className={styles.bulk}>
					<legend className={styles.bulkLegend}>{t('ops.conflict.bulk.label')}</legend>
					<select
						id={bulkId}
						className={styles.select}
						data-danger={answers.bulk !== null && isDestructive(answers.bulk) ? '' : undefined}
						value={answers.bulk ?? ''}
						aria-label={t('ops.conflict.bulk.label')}
						aria-describedby={answers.bulk ? bulkHintId : undefined}
						onChange={(event) => setBulk(event.target.value)}
					>
						<option value="">{t('ops.conflict.bulk.placeholder')}</option>
						{bulkOptions.map((policy) => (
							<option key={policy} value={policy}>
								{policyLabel(policy)}
							</option>
						))}
					</select>
					<label className={styles.later} htmlFor={laterId}>
						<input
							id={laterId}
							type="checkbox"
							checked={answers.later}
							disabled={answers.bulk === null}
							aria-describedby={laterHintId}
							onChange={(event) => setAnswers((now) => ({ ...now, later: event.target.checked }))}
						/>
						{t('ops.conflict.later.label')}
					</label>
					<p id={laterHintId} className={styles.hint}>
						{t('ops.conflict.later.hint')}
					</p>
					{answers.bulk && (
						<div id={bulkHintId} className={styles.bulkNote}>
							<p className={styles.hint}>
								{stats.coveredByBulk === stats.open
									? tf('ops.conflict.bulk.coversAll', { open: stats.open })
									: tf('ops.conflict.bulk.covers', {
											covered: stats.coveredByBulk,
											open: stats.open,
										})}
							</p>
							<p className={styles.hint} data-danger={isDestructive(answers.bulk) ? '' : undefined}>
								{choiceNote(answers.bulk, bulkReplacesFolder, restoring)}
							</p>
						</div>
					)}
				</fieldset>

				<table className={styles.table}>
					<caption className={styles.srOnly}>{t('ops.conflict.table.label')}</caption>
					<thead>
						<tr>
							<th scope="col">{t('ops.conflict.col.name')}</th>
							<th scope="col">{t('ops.conflict.col.existing')}</th>
							<th scope="col">
								{t(restoring ? 'ops.conflict.col.incoming.restore' : 'ops.conflict.col.incoming')}
							</th>
							<th scope="col">{t('ops.conflict.col.choice')}</th>
						</tr>
					</thead>
					<tbody>
						{visible.map((conflict, index) => {
							const noteId = `${baseId}-n${index}`;
							const own = answers.rows.get(conflict.source.uri) ?? '';
							const effective = effectivePolicy(conflict, answers);
							const hint = dateHint(conflict);
							const incomingIsFolder =
								conflict.kind === 'folderOverFolder' || conflict.kind === 'folderOverFile';
							const mismatch =
								conflict.kind === 'fileOverFolder'
									? t('ops.conflict.mismatch.fileOverFolder')
									: conflict.kind === 'folderOverFile'
										? t('ops.conflict.mismatch.folderOverFile')
										: null;
							const covered =
								own === '' && effective !== null
									? tf('ops.conflict.choice.bulk', { choice: policyLabel(effective) })
									: t('ops.conflict.choice.placeholder');
							return (
								<tr
									key={conflict.source.uri}
									className={styles.row}
									data-kind={conflict.kind}
									data-answered={effective ? '' : undefined}
								>
									<th scope="row" className={styles.name}>
										{conflict.name}
									</th>
									<td>
										{conflict.withinBatch ? (
											t('ops.conflict.batch')
										) : (
											<>
												{entryText(
													existingIsFolder(conflict.kind),
													conflict.existingSize,
													conflict.existingModifiedMs,
													hourCycle,
												)}
											</>
										)}
									</td>
									<td>
										{entryText(
											incomingIsFolder,
											conflict.sourceSize,
											conflict.sourceModifiedMs,
											hourCycle,
										)}
										{hint !== 'unknown' && (
											<span className={styles.dateHint} data-hint={hint}>
												{t(`ops.conflict.hint.${hint}` as MessageId)}
											</span>
										)}
									</td>
									<td>
										<select
											className={styles.select}
											data-danger={effective && isDestructive(effective) ? '' : undefined}
											value={own}
											aria-label={tf('ops.conflict.choice.for', { name: conflict.name })}
											aria-describedby={noteId}
											onChange={(event) => setRow(conflict, event.target.value)}
										>
											<option value="">{covered}</option>
											{legalChoices(conflict.kind).map((policy) => (
												<option key={policy} value={policy}>
													{policyLabel(policy)}
												</option>
											))}
										</select>
										<span id={noteId} className={styles.note}>
											{mismatch && <span className={styles.mismatch}>{mismatch}</span>}
											{effective && (
												<span data-danger={isDestructive(effective) ? '' : undefined}>
													{choiceNote(effective, existingIsFolder(conflict.kind), restoring)}
												</span>
											)}
										</span>
									</td>
								</tr>
							);
						})}
					</tbody>
				</table>
				{hidden > 0 && (
					<div className={styles.more}>
						<p>{pluralText('ops.conflict.more', hidden)}</p>
						<p className={styles.hint}>{t('ops.conflict.more.hint')}</p>
						<button type="button" className={styles.showAll} onClick={() => setShowAll(true)}>
							{t('ops.conflict.showAll')}
						</button>
					</div>
				)}
			</div>
			<ConfirmDialog
				open={confirming}
				title={t('ops.conflict.cancelConfirm.title')}
				message={t('ops.conflict.cancelConfirm.message')}
				confirmLabel={t('ops.conflict.cancelConfirm.confirm')}
				cancelLabel={t('ops.conflict.cancelConfirm.keep')}
				danger
				onConfirm={() => {
					setConfirming(false);
					onCancelJob();
				}}
				onCancel={() => setConfirming(false)}
			/>
		</Dialog>
	);
}
