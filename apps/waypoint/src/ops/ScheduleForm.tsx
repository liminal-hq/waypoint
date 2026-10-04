// The form that sets when a waiting job may start: at a time, or only inside a daily window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useId, useRef, useState, type FormEvent } from 'react';
import { t } from '../i18n/messages';
import type { Schedule } from '../services/opsClient';
import styles from './OpsPanel.module.css';
import {
	SCHEDULE_PROBLEM_MESSAGES,
	draftOf,
	scheduleFromDraft,
	type ScheduleDraft,
	type ScheduleProblem,
} from './scheduleModel';

interface ScheduleFormProps {
	/** What the job has now, or `null`. */
	current: Schedule | null;
	/** The label the group is named by: "Schedule: Copying a". */
	label: string;
	now: () => number;
	onApply: (schedule: Schedule) => void;
	/** "Run now": clears the schedule. Offered only for a job that has one. */
	onRunNow: () => void;
	onCancel: () => void;
}

export function ScheduleForm({
	current,
	label,
	now,
	onApply,
	onRunNow,
	onCancel,
}: ScheduleFormProps) {
	const id = useId();
	const form = useRef<HTMLFormElement | null>(null);
	// The keyboard moves into the form when it opens, and Escape (handled below) brings it back out.
	useEffect(() => {
		form.current?.querySelector<HTMLElement>('input:checked')?.focus();
	}, []);
	const [draft, setDraft] = useState<ScheduleDraft>(() => draftOf(current, now()));
	const [problem, setProblem] = useState<ScheduleProblem | null>(null);
	const change = (patch: Partial<ScheduleDraft>) => {
		setDraft((d) => ({ ...d, ...patch }));
		setProblem(null);
	};
	const submit = (event: FormEvent) => {
		event.preventDefault();
		const result = scheduleFromDraft(draft, now());
		if ('problem' in result) setProblem(result.problem);
		else onApply(result.schedule);
	};
	const errorId = `${id}-error`;
	return (
		<form
			ref={form}
			className={styles.scheduleForm}
			role="group"
			aria-label={label}
			onSubmit={submit}
			onKeyDown={(event) => {
				if (event.key === 'Escape') {
					event.stopPropagation();
					onCancel();
				}
			}}
		>
			<fieldset className={styles.fieldset}>
				<legend className={styles.legend}>{t('ops.schedule.mode.label')}</legend>
				<label className={styles.control}>
					<input
						type="radio"
						name={`${id}-mode`}
						checked={draft.mode === 'startAt'}
						onChange={() => change({ mode: 'startAt' })}
					/>
					<span>{t('ops.schedule.mode.startAt')}</span>
				</label>
				<label className={styles.control}>
					<input
						type="radio"
						name={`${id}-mode`}
						checked={draft.mode === 'window'}
						onChange={() => change({ mode: 'window' })}
					/>
					<span>{t('ops.schedule.mode.window')}</span>
				</label>
			</fieldset>
			{draft.mode === 'startAt' ? (
				<label className={styles.control}>
					<span>{t('ops.schedule.startAt.label')}</span>
					<input
						type="datetime-local"
						className={styles.select}
						value={draft.startAt}
						aria-invalid={problem === 'past' || problem === 'incomplete'}
						aria-describedby={problem ? errorId : undefined}
						onChange={(event) => change({ startAt: event.target.value })}
					/>
				</label>
			) : (
				<div className={styles.controls}>
					<label className={styles.control}>
						<span>{t('ops.schedule.from')}</span>
						<input
							type="time"
							className={styles.select}
							value={draft.from}
							aria-invalid={problem === 'same' || problem === 'incomplete'}
							aria-describedby={problem ? errorId : undefined}
							onChange={(event) => change({ from: event.target.value })}
						/>
					</label>
					<label className={styles.control}>
						<span>{t('ops.schedule.until')}</span>
						<input
							type="time"
							className={styles.select}
							value={draft.until}
							aria-invalid={problem === 'same' || problem === 'incomplete'}
							aria-describedby={problem ? errorId : undefined}
							onChange={(event) => change({ until: event.target.value })}
						/>
					</label>
				</div>
			)}
			{problem && (
				<p id={errorId} className={styles.problem} role="alert">
					{t(SCHEDULE_PROBLEM_MESSAGES[problem])}
				</p>
			)}
			<div className={styles.actions}>
				<button type="submit" className={styles.textButton}>
					{t('ops.schedule.apply')}
				</button>
				{current && (
					<button type="button" className={styles.textButton} onClick={onRunNow}>
						{t('ops.schedule.runNow')}
					</button>
				)}
				<button type="button" className={styles.textButton} onClick={onCancel}>
					{t('ops.schedule.cancel')}
				</button>
			</div>
		</form>
	);
}
