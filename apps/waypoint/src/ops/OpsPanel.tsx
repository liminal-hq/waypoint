// The list of jobs, shared by the status bar popover and the Operations window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useId, useRef, useState, type KeyboardEvent } from 'react';
import { showNotice } from '../app/notices';
import { t, tf } from '../i18n/messages';
import { announce } from '../tabs/announcer';
import type { JobView, JobAction } from './jobText';
import { useOps, useOpsViews } from './OpsContext';
import { OpsDownIcon, OpsPopOutIcon, OpsUpIcon } from './OpsIcons';
import styles from './OpsPanel.module.css';
import { requestResolve } from './resolveHook';

function failureText(error: unknown): string {
	if (error && typeof error === 'object' && 'message' in error) {
		return String((error as { message: unknown }).message);
	}
	return String(error);
}

interface OpsPanelProps {
	/** `popover` has the Pop out button and a heading; `window` fills the Operations window. */
	layout: 'popover' | 'window';
	/** Called after an action that should close a popover (Pop out). */
	onDone?: () => void;
	/** Focus goes to the first row when this is set at mount, to the panel itself otherwise. */
	autoFocus?: boolean;
}

/**
 * The jobs in queue order. The rows are a list with one tab stop: Up and Down move between them,
 * Home and End jump, and Alt+Up and Alt+Down move a queued job (the keyboard path for reordering).
 * Each row's buttons are ordinary buttons reached with Tab.
 */
export function OpsPanel({ layout, onDone, autoFocus = false }: OpsPanelProps) {
	const ops = useOps();
	const views = useOpsViews();
	const headingId = useId();
	const hintId = useId();
	const rows = useRef(new Map<number, HTMLLIElement>());
	const container = useRef<HTMLDivElement | null>(null);
	const [current, setCurrent] = useState<number | null>(null);
	const refocus = useRef<number | null>(null);

	const active = views.find((v) => v.id === current) ?? views[0];

	// Focus the first row when the panel opens as a popover.
	useEffect(() => {
		if (!autoFocus) return;
		const first = container.current?.querySelector<HTMLElement>('[data-job-row]');
		(first ?? container.current)?.focus();
	}, [autoFocus]);

	// After a reorder the row sits somewhere else: keep the keyboard on it.
	useEffect(() => {
		if (refocus.current === null) return;
		rows.current.get(refocus.current)?.focus();
		refocus.current = null;
	});

	if (!ops) return null;
	const { handle, popOut, showInFolder } = ops;
	const finishedCount = views.filter((v) => v.finished).length;

	const guard = (work: Promise<unknown>) =>
		work.catch((error: unknown) => showNotice(failureText(error)));

	const perform = (view: JobView, action: JobAction) => {
		const { client } = handle;
		switch (action) {
			case 'pause':
				return guard(client.pause(view.id));
			case 'resume':
				return guard(client.resume(view.id));
			case 'cancel':
				return guard(client.cancel(view.id));
			case 'retry':
				return guard(client.retry(view.id));
			case 'dismiss':
				return guard(client.dismiss(view.id));
			case 'resolve':
				return requestResolve(view.id);
			case 'showInFolder':
				if (view.destination) showInFolder?.(view.destination);
				return;
		}
	};

	const move = (view: JobView, by: -1 | 1) => {
		if (view.queuePosition === null) return;
		const to = view.queuePosition + by;
		if (to < 0) return;
		refocus.current = view.id;
		void guard(handle.client.reorder(view.id, to));
		announce(tf('ops.reordered', { title: view.title, position: to + 1 }));
	};

	const onRowKeyDown = (event: KeyboardEvent<HTMLLIElement>, view: JobView, index: number) => {
		if (event.target !== event.currentTarget) return;
		if (event.altKey && (event.key === 'ArrowUp' || event.key === 'ArrowDown')) {
			event.preventDefault();
			move(view, event.key === 'ArrowUp' ? -1 : 1);
			return;
		}
		let next: number | null = null;
		if (event.key === 'ArrowUp') next = Math.max(0, index - 1);
		else if (event.key === 'ArrowDown') next = Math.min(views.length - 1, index + 1);
		else if (event.key === 'Home') next = 0;
		else if (event.key === 'End') next = views.length - 1;
		if (next === null) return;
		event.preventDefault();
		const target = views[next];
		if (target) {
			setCurrent(target.id);
			rows.current.get(target.id)?.focus();
		}
	};

	return (
		<div
			ref={container}
			className={styles.panel}
			role="group"
			data-layout={layout}
			tabIndex={-1}
			aria-labelledby={headingId}
		>
			<header className={styles.header}>
				<h2 id={headingId} className={styles.heading}>
					{t('ops.panel.label')}
				</h2>
				<div className={styles.headerActions}>
					<button
						type="button"
						className={styles.textButton}
						disabled={finishedCount === 0}
						onClick={() => void guard(handle.client.dismissFinished())}
					>
						{t('ops.clearFinished')}
					</button>
					{layout === 'popover' && popOut && (
						<button
							type="button"
							className={styles.textButton}
							onClick={() => {
								popOut();
								onDone?.();
							}}
						>
							<OpsPopOutIcon width={14} height={14} />
							{t('ops.popOut')}
						</button>
					)}
				</div>
			</header>
			{views.length === 0 ? (
				<p className={styles.empty}>{t('ops.list.empty')}</p>
			) : (
				<>
					<ul className={styles.list} aria-label={t('ops.list.label')} aria-describedby={hintId}>
						{views.map((view, index) => {
							const titleId = `${headingId}-t${view.id}`;
							const stateId = `${headingId}-s${view.id}`;
							return (
								<li
									key={view.id}
									ref={(element) => {
										if (element) rows.current.set(view.id, element);
										else rows.current.delete(view.id);
									}}
									className={styles.row}
									data-job-row=""
									data-job={view.id}
									data-state={view.state}
									data-attention={view.attention ?? undefined}
									tabIndex={active?.id === view.id ? 0 : -1}
									aria-labelledby={titleId}
									aria-describedby={stateId}
									onFocus={(event) => {
										if (event.target === event.currentTarget) setCurrent(view.id);
									}}
									onKeyDown={(event) => onRowKeyDown(event, view, index)}
								>
									<div className={styles.rowMain}>
										<span id={titleId} className={styles.title}>
											{view.title}
										</span>
										{view.route && <span className={styles.route}>{view.route}</span>}
										<span id={stateId} className={styles.state}>
											{view.stateText}
										</span>
										{view.showProgress && (
											<div
												className={styles.bar}
												role="progressbar"
												aria-label={view.title}
												aria-valuemin={0}
												aria-valuemax={100}
												aria-valuenow={
													view.fraction === null ? undefined : Math.round(view.fraction * 100)
												}
											>
												<div
													className={styles.barFill}
													style={{ width: `${Math.round((view.fraction ?? 0) * 100)}%` }}
												/>
											</div>
										)}
										{view.detail && <span className={styles.detail}>{view.detail}</span>}
									</div>
									<div className={styles.actions}>
										{view.queuePosition !== null && (
											<>
												<button
													type="button"
													className={styles.iconButton}
													disabled={!view.canMoveUp}
													aria-label={tf('ops.action.for', {
														action: t('ops.moveUp'),
														title: view.title,
													})}
													title={t('ops.moveUp')}
													onClick={() => move(view, -1)}
												>
													<OpsUpIcon width={14} height={14} />
												</button>
												<button
													type="button"
													className={styles.iconButton}
													disabled={!view.canMoveDown}
													aria-label={tf('ops.action.for', {
														action: t('ops.moveDown'),
														title: view.title,
													})}
													title={t('ops.moveDown')}
													onClick={() => move(view, 1)}
												>
													<OpsDownIcon width={14} height={14} />
												</button>
											</>
										)}
										{view.actions.map((action) => (
											<button
												key={action}
												type="button"
												className={styles.textButton}
												data-action={action}
												aria-label={tf('ops.action.for', {
													action: t(`ops.action.${action}`),
													title: view.title,
												})}
												onClick={() => void perform(view, action)}
											>
												{t(`ops.action.${action}`)}
											</button>
										))}
									</div>
								</li>
							);
						})}
					</ul>
					<p id={hintId} className={styles.hint}>
						{t('ops.list.hint')}
					</p>
				</>
			)}
		</div>
	);
}
