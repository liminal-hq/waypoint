// The Operations list's interrupted transfers: what a lost connection stopped in an earlier run, each with Resume and Discard…
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ResumableRecord } from '@liminal-hq/waypoint-protocol/generated/ResumableRecord';
import { useCallback, useEffect, useId, useState } from 'react';
import { showNotice } from '../app/notices';
import { t, tf, tn } from '../i18n/messages';
import type { OpsClient } from '../services/opsClient';
import type { ConfirmSpec } from './fileCommands';
import { discardInterrupted, resumeInterrupted } from './interrupted';
import styles from './OpsPanel.module.css';

type Client = Pick<OpsClient, 'interruptedTransfers' | 'resumeInterrupted' | 'discardInterrupted'>;

interface InterruptedTransfersProps {
	client: Client;
	/** Asks before Discard removes anything. */
	confirm: (spec: ConfirmSpec) => Promise<boolean>;
	/** Changes when the queue does, so a resumed transfer's record is read again once it ends. */
	revision: string;
}

/**
 * Lists every transfer a lost connection stopped in an earlier run (D165), oldest first, nothing
 * when there is none. Resume runs it again from where it stopped; Discard… asks, naming every file
 * whose partial copy goes, then removes them. Nothing here resumes by itself.
 */
export function InterruptedTransfers({ client, confirm, revision }: InterruptedTransfersProps) {
	const [records, setRecords] = useState<ResumableRecord[]>([]);
	const headingId = useId();
	const load = useCallback(() => {
		client.interruptedTransfers().then(setRecords, (error: unknown) => {
			console.warn('could not read the interrupted transfers', error);
		});
	}, [client]);
	useEffect(load, [load, revision]);
	if (records.length === 0) return null;

	const act = async (work: Promise<boolean>) => {
		if (await work) load();
	};

	return (
		<section aria-labelledby={headingId}>
			<h3 id={headingId} className={styles.heading}>
				{t('ops.interrupted.heading')}
			</h3>
			<ul className={styles.list}>
				{records.map((record) => (
					<li key={`${record.job}-${record.atMs}`} className={styles.row} data-interrupted="">
						<div className={styles.rowMain}>
							<span className={styles.title}>{record.label}</span>
							<span className={styles.detail}>
								{tn('ops.interrupted.detail', record.points.length)}
							</span>
						</div>
						<div className={styles.actions}>
							<button
								type="button"
								className={styles.textButton}
								aria-label={tf('ops.action.for', {
									action: t('ops.recovery.resume'),
									title: record.label,
								})}
								onClick={() => void act(resumeInterrupted(client, record, showNotice))}
							>
								{t('ops.recovery.resume')}
							</button>
							<button
								type="button"
								className={styles.textButton}
								aria-label={tf('ops.action.for', {
									action: t('ops.interrupted.discardEllipsis'),
									title: record.label,
								})}
								onClick={() => void act(discardInterrupted(client, record, confirm, showNotice))}
							>
								{t('ops.interrupted.discardEllipsis')}
							</button>
						</div>
					</li>
				))}
			</ul>
		</section>
	);
}
