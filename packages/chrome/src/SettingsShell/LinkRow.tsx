// Settings row that is a link to another page or an external address
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MouseEvent, ReactNode } from 'react';
import '../tokens.css';
import { ChevronRightIcon } from '../icons/icons';
import { useSettingsLabels } from './labels';
import styles from './SettingsRow.module.css';

export interface LinkRowProps {
	label: ReactNode;
	description?: ReactNode;
	href: string;
	/**
	 * Called on activation. When given, the default navigation is cancelled so the app can open the
	 * address itself (in the system browser, say) or move to another settings page.
	 */
	onActivate?: (event: MouseEvent<HTMLAnchorElement>) => void;
	unavailableReason?: string;
}

export function LinkRow({ label, description, href, onActivate, unavailableReason }: LinkRowProps) {
	const labels = useSettingsLabels();
	const unavailable = unavailableReason !== undefined;
	const content = (
		<>
			<span className={styles.text}>
				<span className={styles.label}>{label}</span>
				{description ? <span className={styles.description}>{description}</span> : null}
				{unavailable ? (
					<span className={styles.reason}>
						{labels.unavailable}: {unavailableReason}
					</span>
				) : null}
			</span>
			<ChevronRightIcon className={styles.chevron} />
		</>
	);
	if (unavailable) {
		return (
			<div
				className={`${styles.row} ${styles.linkRow} ${styles.off} ${styles.unavailable}`}
				aria-disabled="true"
			>
				{content}
			</div>
		);
	}
	return (
		<a
			className={`${styles.row} ${styles.linkRow}`}
			href={href}
			onClick={(event) => {
				if (!onActivate) return;
				event.preventDefault();
				onActivate(event);
			}}
		>
			{content}
		</a>
	);
}
