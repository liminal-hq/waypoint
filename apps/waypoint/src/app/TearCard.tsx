// The mini window card a tear-off drag shows: the ghost window draws it, and so does the page where no ghost can follow
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { GhostPayload } from '../services/tearoffClient';
import styles from './TearCard.module.css';

/** The first tab's folder, a count for a group or a pair, and what a release would do. */
export function TearCard({ payload, testId }: { payload: GhostPayload; testId?: string }) {
	return (
		<div className={styles.card} data-testid={testId}>
			{payload.count !== undefined && <span className={styles.count}>{payload.count}</span>}
			<div className={styles.text}>
				{payload.title !== undefined && <span className={styles.title}>{payload.title}</span>}
				{payload.label !== undefined && <span className={styles.label}>{payload.label}</span>}
			</div>
		</div>
	);
}
