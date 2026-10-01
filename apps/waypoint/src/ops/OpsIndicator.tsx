// The ring in the status bar: overall progress and how many jobs are in progress, opening the queue popover
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useId, useRef, useState } from 'react';
import { useOps, useOpsJobs } from './OpsContext';
import { OpsCheckIcon, OpsQueueIcon } from './OpsIcons';
import styles from './OpsIndicator.module.css';
import { OpsPopover } from './OpsPopover';
import { ringModel } from './opsRing';

const RADIUS = 7;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;
/** What a ring with jobs that are not sized yet shows: a still quarter, never a spinner. */
const UNSIZED = 0.25;

/**
 * Always in the bar once the window has a queue. Quiet when nothing is listed, a ring that fills as
 * the jobs do while any run, a dot when one waits for the person (attention) or has failed. It
 * never animates beyond a short fill transition that reduced motion turns off.
 */
export function OpsIndicator() {
	const ops = useOps();
	const jobs = useOpsJobs();
	const [open, setOpen] = useState(false);
	const wrap = useRef<HTMLDivElement | null>(null);
	const button = useRef<HTMLButtonElement | null>(null);
	const popoverId = useId();
	if (!ops) return null;

	const ring = ringModel(jobs);
	const filled = ring.count > 0 ? (ring.fraction ?? UNSIZED) : 0;
	const attention = ring.urgency === 'waiting' || ring.urgency === 'failed' ? ring.urgency : null;
	const showRing = ring.count > 0;

	return (
		<div ref={wrap} className={styles.wrap}>
			<button
				ref={button}
				type="button"
				className={styles.button}
				data-ops-ring=""
				data-urgency={ring.urgency}
				aria-label={ring.label}
				aria-haspopup="dialog"
				aria-expanded={open}
				aria-controls={open ? popoverId : undefined}
				onClick={() => setOpen((was) => !was)}
			>
				<span className={styles.glyph}>
					{showRing ? (
						<svg className={styles.ring} viewBox="0 0 18 18" aria-hidden="true" focusable="false">
							<circle className={styles.track} cx="9" cy="9" r={RADIUS} />
							<circle
								className={styles.arc}
								cx="9"
								cy="9"
								r={RADIUS}
								strokeDasharray={`${CIRCUMFERENCE * filled} ${CIRCUMFERENCE}`}
								data-fraction={ring.fraction === null ? undefined : filled.toFixed(3)}
							/>
						</svg>
					) : ring.urgency === 'done' ? (
						<OpsCheckIcon width={14} height={14} />
					) : (
						<OpsQueueIcon width={14} height={14} />
					)}
					{attention && <span className={styles.dot} data-kind={attention} />}
				</span>
				{ring.count > 0 && <span className={styles.count}>{ring.count}</span>}
			</button>
			{open && (
				<OpsPopover
					id={popoverId}
					boundary={wrap}
					onClose={(reason) => {
						setOpen(false);
						if (reason === 'escape') button.current?.focus();
					}}
				/>
			)}
		</div>
	);
}
