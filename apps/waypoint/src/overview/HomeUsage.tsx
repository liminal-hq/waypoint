// "Biggest folders in Home": the top-level folders of Home with size bars, filling in as the scan reports them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { DirSizeRow } from '@liminal-hq/waypoint-protocol/generated/DirSizeRow';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { useEffect, useId, useRef } from 'react';
import { formatSize } from '../browse/format';
import { t, tf, tn } from '../i18n/messages';
import type { HomeUsage as HomeUsageState } from './homeMeasure';
import styles from './HomeUsage.module.css';

interface HomeUsageProps {
	usage: HomeUsageState;
	/** "Wed, 2:14 p.m." for the cached or finished result, or `null` when there is none. */
	asOf: string | null;
	/** Opens a folder in the tab. */
	onOpen(location: Location): void;
	/** Says something politely to a screen reader. */
	announce(message: string): void;
}

/** A row's share as whole percent, kept apart from zero when the row is not empty. */
export function sharePercent(row: Pick<DirSizeRow, 'bytes' | 'share'>): number {
	const percent = Math.round(row.share * 100);
	return percent === 0 && row.bytes > 0 ? 0.5 : percent;
}

function shareText(percent: number): string {
	return percent < 1 && percent > 0
		? t('overview.home.row.shareSmall')
		: tf('overview.home.row.share', { percent });
}

/**
 * The rows arrive folder by folder, largest first, with "Other files and folders, including
 * hidden" last. Each folder is a button (Enter opens it in the tab) whose name says everything the
 * bar draws: "Backup, 128 GB, 21% of Home". The bar is only decoration: the size and the share are
 * written in the row, and the remainder's bar is hatched rather than solid. The page's live
 * region gets the start, each quarter of the folders, and the end.
 */
export function HomeUsage({ usage, asOf, onOpen, announce }: HomeUsageProps) {
	const titleId = useId();
	const { scan } = usage;
	const shown = scan.shown;
	const rows = shown?.rows ?? [];
	const done = shown?.foldersScanned ?? 0;
	const total = shown?.foldersTotal ?? 0;

	// What the screen reader hears: the start, each quarter, and the end. `spoken` remembers the quarter.
	const spoken = useRef({ running: false, quarter: 0 });
	useEffect(() => {
		const memory = spoken.current;
		if (scan.running) {
			if (!memory.running) {
				memory.running = true;
				memory.quarter = 0;
				announce(t('overview.home.announce.start'));
				return;
			}
			const quarter = total > 0 ? Math.floor((done / total) * 4) : 0;
			if (quarter > memory.quarter && quarter < 4) {
				memory.quarter = quarter;
				announce(tf('overview.home.announce.progress', { percent: quarter * 25 }));
			}
			return;
		}
		if (!memory.running) return;
		memory.running = false;
		if (scan.outcome === 'done' && scan.final) {
			announce(tf('overview.home.announce.done', { size: formatSize(scan.final.totalBytes) }));
		} else if (scan.outcome === 'cancelled') {
			announce(t('overview.home.announce.cancelled'));
		} else if (scan.outcome === 'failed') {
			announce(tf('overview.home.failed', { reason: scan.error ?? '' }));
		}
	}, [scan.running, scan.outcome, scan.final, scan.error, done, total, announce]);

	const status = scan.running
		? total > 0
			? tf('overview.home.progress', { done, total })
			: t('overview.home.progress.unknown')
		: scan.outcome === 'failed'
			? tf('overview.home.failed', { reason: scan.error ?? '' })
			: scan.outcome === 'cancelled'
				? t('overview.home.cancelled')
				: asOf !== null && scan.final
					? tf('overview.home.asOf', { time: asOf })
					: rows.length === 0
						? t('overview.home.notMeasured')
						: '';
	const placeholders = shown?.placeholders ?? 0;

	return (
		<section
			className={styles.section}
			aria-labelledby={titleId}
			aria-busy={scan.running || undefined}
		>
			<header className={styles.header}>
				<h3 id={titleId} className={styles.title}>
					{t('overview.home.title')}
				</h3>
				<span className={styles.status} data-outcome={scan.running ? 'running' : scan.outcome}>
					{status}
				</span>
				{scan.running ? (
					<button
						type="button"
						className={styles.action}
						aria-label={t('overview.home.cancel.label')}
						onClick={usage.cancel}
					>
						{t('overview.home.cancel')}
					</button>
				) : (
					<button
						type="button"
						className={styles.action}
						aria-label={t('overview.home.measureNow.label')}
						onClick={usage.measureNow}
					>
						{t('overview.home.measureNow')}
					</button>
				)}
			</header>
			{rows.length > 0 && (
				<ul className={styles.rows} aria-label={t('overview.home.rows.label')}>
					{rows.map((row) => (
						<HomeRow key={`${row.kind}:${row.name}`} row={row} onOpen={onOpen} />
					))}
				</ul>
			)}
			{rows.length === 0 && !scan.running && scan.final && (
				<p className={styles.quiet}>{t('overview.home.empty')}</p>
			)}
			{placeholders > 0 && (
				<p className={styles.quiet} role="note">
					{tn('overview.home.placeholders', placeholders)}
				</p>
			)}
		</section>
	);
}

function HomeRow({ row, onOpen }: { row: DirSizeRow; onOpen(location: Location): void }) {
	const remainder = row.kind === 'other';
	const name = remainder ? t('overview.home.remainder') : row.name;
	const size = formatSize(row.bytes);
	const percent = sharePercent(row);
	const body = (
		<>
			<span className={styles.name}>{name}</span>
			<span className={styles.bar} aria-hidden="true">
				<span
					className={styles.fill}
					data-kind={row.kind}
					style={{ inlineSize: `${Math.min(100, Math.max(0, row.share * 100))}%` }}
				/>
			</span>
			<span className={styles.size}>{size}</span>
			<span className={styles.share}>{shareText(percent)}</span>
		</>
	);
	return (
		<li className={styles.item} data-kind={row.kind}>
			{row.location && !remainder ? (
				<button
					type="button"
					className={styles.row}
					title={tf('overview.home.row.open', { name })}
					aria-label={tf('overview.home.row.label', {
						name,
						size,
						percent: percent < 1 ? t('overview.home.row.underOne') : percent,
					})}
					onClick={() => onOpen(row.location as Location)}
				>
					{body}
				</button>
			) : (
				<div className={styles.row}>{body}</div>
			)}
		</li>
	);
}
