// The batch rename dialog: a stack of rules, a live before and after table, and Apply once nothing is wrong
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Dialog } from '@liminal-hq/waypoint-chrome/Dialog/Dialog';
import { DialogActions, DialogButton } from '@liminal-hq/waypoint-chrome/Dialog/DialogActions';
import type { BatchPreview } from '@liminal-hq/waypoint-protocol/generated/BatchPreview';
import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { t, tf, tn } from '../../i18n/messages';
import { errorText, type BatchRenameApi, type BatchRenameSelection } from './batchRenameApi';
import {
	addRule,
	buildRequest,
	changesAnExtension,
	defaultRule,
	hasBlockingProblem,
	isReady,
	localUtcOffsetMinutes,
	moveRule,
	removeRule,
	RULE_KINDS,
	updateRule,
	visibleRows,
	type KeyedRule,
	type RuleKind,
} from './batchRenameModel';
import { rowNotes, summaryText } from './batchRenameText';
import styles from './BatchRenameDialog.module.css';
import { RuleForm } from './RuleForm';

export type BatchRenameResult = 'cancelled' | 'applied';

export interface BatchRenameDialogProps {
	open: boolean;
	selection: BatchRenameSelection;
	api: BatchRenameApi;
	/** The dialog is done: the person cancelled, or the job was queued. */
	onClose: (result: BatchRenameResult) => void;
	/** Tells the window what happened once the dialog has gone (its own live region goes with it). */
	announce?: (message: string) => void;
	/** How long typing must pause before the preview is asked for again. */
	debounceMs?: number;
	/** Local time's distance ahead of UTC, for the dates rules write; defaults to this machine's. */
	utcOffsetMinutes?: number;
}

type Status = 'checking' | 'ready' | 'failed';

export function BatchRenameDialog(props: BatchRenameDialogProps) {
	// Mounted only while open, so each opening starts with a fresh stack.
	return props.open ? <OpenBatchRename {...props} /> : null;
}

function OpenBatchRename({
	selection,
	api,
	onClose,
	announce,
	debounceMs = 200,
	utcOffsetMinutes,
}: BatchRenameDialogProps) {
	const nextId = useRef(2);
	const [stack, setStack] = useState<KeyedRule[]>(() => [
		{ id: 1, rule: defaultRule('findReplace') },
	]);
	const [addKind, setAddKind] = useState<RuleKind>('findReplace');
	const [preview, setPreview] = useState<BatchPreview | null>(null);
	const [status, setStatus] = useState<Status>('checking');
	const [failure, setFailure] = useState('');
	const [applying, setApplying] = useState(false);
	const [applyError, setApplyError] = useState<string | null>(null);
	const offset = useMemo(() => utcOffsetMinutes ?? localUtcOffsetMinutes(), [utcOffsetMinutes]);
	const sources = selection.sources;

	// Asks Rust what the stack does, a moment after the last change. An answer for a stack that
	// has since changed is dropped.
	useEffect(() => {
		let stale = false;
		setStatus('checking');
		const timer = setTimeout(() => {
			const request = buildRequest(
				sources,
				stack.map((entry) => entry.rule),
				offset,
			);
			api.preview(request).then(
				(answer) => {
					if (stale) return;
					setPreview(answer);
					setFailure('');
					setStatus('ready');
				},
				(error: unknown) => {
					if (stale) return;
					setFailure(errorText(error));
					setStatus('failed');
				},
			);
		}, debounceMs);
		return () => {
			stale = true;
			clearTimeout(timer);
		};
	}, [api, sources, stack, offset, debounceMs]);

	const ready = status === 'ready' && isReady(preview);
	const count = selection.count ?? preview?.rows.length ?? 0;

	const apply = async () => {
		if (!ready || !preview || applying) return;
		setApplying(true);
		setApplyError(null);
		try {
			// The job carries the time the preview used, so it writes the names that were shown.
			await api.apply(
				buildRequest(
					sources,
					stack.map((entry) => entry.rule),
					offset,
					preview.nowMs,
				),
			);
			announce?.(tn('batchRename.applied', preview.changes));
			onClose('applied');
		} catch (error) {
			setApplyError(tf('batchRename.applyFailed', { reason: errorText(error) }));
			setApplying(false);
		}
	};

	const shown = preview ? visibleRows(preview) : null;
	const ruleErrors = preview?.ruleErrors ?? [];

	return (
		<Dialog
			open
			size="large"
			title={t('batchRename.title')}
			description={count > 0 ? tn('batchRename.selection', count) : undefined}
			initialFocus="[data-batch-initial]"
			onClose={() => {
				if (!applying) onClose('cancelled');
			}}
			footer={
				<DialogActions>
					<DialogButton closes disabled={applying}>
						{t('batchRename.cancel')}
					</DialogButton>
					<DialogButton variant="primary" disabled={!ready || applying} onClick={apply}>
						{applying ? t('batchRename.applying') : t('batchRename.apply')}
					</DialogButton>
				</DialogActions>
			}
		>
			<div className={styles.layout}>
				<RuleStack
					stack={stack}
					addKind={addKind}
					onAddKind={setAddKind}
					errors={ruleErrors}
					onChange={setStack}
					onAdd={() => {
						setStack((current) => addRule(current, addKind, nextId.current++));
					}}
				/>
				<section className={styles.preview} aria-labelledby="batch-rename-preview-heading">
					<h3 id="batch-rename-preview-heading" className={styles.sectionHeading}>
						{t('batchRename.preview.label')}
					</h3>
					<div className={styles.summary} role="status" aria-live="polite">
						{status === 'failed'
							? tf('batchRename.preview.failed', { reason: failure })
							: preview
								? summaryText(preview)
								: t('batchRename.preview.checking')}
					</div>
					{applyError ? (
						<div className={styles.error} role="alert">
							{applyError}
						</div>
					) : null}
					{changesAnExtension(preview) ? (
						<p className={styles.extensionNote}>{t('batchRename.extensionNote')}</p>
					) : null}
					{preview && shown ? (
						<>
							<div className={styles.tableWrap}>
								<table
									className={styles.table}
									aria-label={t('batchRename.preview.label')}
									aria-busy={status === 'checking'}
								>
									<thead>
										<tr>
											<th scope="col">{t('batchRename.preview.before')}</th>
											<th scope="col">{t('batchRename.preview.after')}</th>
											<th scope="col">{t('batchRename.preview.note')}</th>
										</tr>
									</thead>
									<tbody>
										{shown.rows.map((row) => (
											<tr
												key={row.index}
												data-problem={hasBlockingProblem(row) ? 'true' : undefined}
												data-unchanged={row.changed ? undefined : 'true'}
											>
												<td className={styles.name}>{row.from}</td>
												<td className={styles.name}>{row.to}</td>
												<td className={styles.notes}>
													{rowNotes(row, preview.rows).map((note) => (
														<div key={note}>{note}</div>
													))}
												</td>
											</tr>
										))}
									</tbody>
								</table>
							</div>
							{shown.hidden > 0 ? (
								<p className={styles.capped}>
									{tf('batchRename.preview.capped', {
										shown: shown.rows.length,
										total: preview.rows.length,
									})}
								</p>
							) : null}
						</>
					) : null}
				</section>
			</div>
		</Dialog>
	);
}

interface RuleStackProps {
	stack: KeyedRule[];
	addKind: RuleKind;
	onAddKind: (kind: RuleKind) => void;
	errors: BatchPreview['ruleErrors'];
	onChange: (stack: KeyedRule[]) => void;
	onAdd: () => void;
}

function RuleStack({ stack, addKind, onAddKind, errors, onChange, onAdd }: RuleStackProps) {
	const headingId = useId();
	return (
		<section className={styles.rules} aria-labelledby={headingId}>
			<h3 id={headingId} className={styles.sectionHeading}>
				{t('batchRename.rules.label')}
			</h3>
			<ol className={styles.stack}>
				{stack.map((entry, at) => {
					const number = at + 1;
					const own = errors.filter((error) => error.rule === at);
					return (
						<li key={entry.id} className={styles.card}>
							<RuleCard
								entry={entry}
								number={number}
								first={at === 0}
								last={at === stack.length - 1}
								errors={own.map((error) =>
									tf('batchRename.rule.error', { number, reason: error.reason }),
								)}
								onChange={(rule) => onChange(updateRule(stack, entry.id, rule))}
								onMove={(by) => onChange(moveRule(stack, entry.id, by))}
								onRemove={() => onChange(removeRule(stack, entry.id))}
							/>
						</li>
					);
				})}
			</ol>
			<div className={styles.addRow}>
				<select
					className={styles.select}
					aria-label={t('batchRename.rule.addType')}
					value={addKind}
					onChange={(event) => onAddKind(event.target.value as RuleKind)}
				>
					{RULE_KINDS.map((kind) => (
						<option key={kind} value={kind}>
							{t(`batchRename.kind.${kind}`)}
						</option>
					))}
				</select>
				<button type="button" className={styles.button} onClick={onAdd}>
					{t('batchRename.rule.add')}
				</button>
			</div>
		</section>
	);
}

interface RuleCardProps {
	entry: KeyedRule;
	number: number;
	first: boolean;
	last: boolean;
	errors: string[];
	onChange: (rule: KeyedRule['rule']) => void;
	onMove: (by: -1 | 1) => void;
	onRemove: () => void;
}

function RuleCard({
	entry,
	number,
	first,
	last,
	errors,
	onChange,
	onMove,
	onRemove,
}: RuleCardProps) {
	const headingId = useId();
	return (
		<section aria-labelledby={headingId}>
			<header className={styles.cardHeader}>
				<h4 id={headingId} className={styles.cardHeading}>
					{tf('batchRename.rule.heading', { number })}
				</h4>
				<select
					className={styles.select}
					aria-label={t('batchRename.rule.type')}
					value={entry.rule.kind}
					data-batch-initial={first ? '' : undefined}
					onChange={(event) => onChange(defaultRule(event.target.value as RuleKind))}
				>
					{RULE_KINDS.map((kind) => (
						<option key={kind} value={kind}>
							{t(`batchRename.kind.${kind}`)}
						</option>
					))}
				</select>
				<span className={styles.spacer} />
				<button
					type="button"
					className={styles.iconButton}
					aria-label={tf('batchRename.rule.moveUp', { number })}
					disabled={first}
					onClick={() => onMove(-1)}
				>
					<span aria-hidden="true">↑</span>
				</button>
				<button
					type="button"
					className={styles.iconButton}
					aria-label={tf('batchRename.rule.moveDown', { number })}
					disabled={last}
					onClick={() => onMove(1)}
				>
					<span aria-hidden="true">↓</span>
				</button>
				<button
					type="button"
					className={styles.iconButton}
					aria-label={tf('batchRename.rule.remove', { number })}
					onClick={onRemove}
				>
					<span aria-hidden="true">×</span>
				</button>
			</header>
			<RuleForm rule={entry.rule} onChange={onChange} />
			{errors.map((message) => (
				<p key={message} className={styles.ruleError} role="alert">
					{t('batchRename.problem.label')}: {message}
				</p>
			))}
		</section>
	);
}
