// Body of one settings page: an optional intro followed by groups
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ReactNode } from 'react';
import '../tokens.css';
import styles from './SettingsGroup.module.css';

export interface SettingsSectionProps {
	/** Introductory text above the first group. */
	description?: ReactNode;
	children: ReactNode;
}

/** The body of one settings page: an optional intro followed by groups. */
export function SettingsSection({ description, children }: SettingsSectionProps) {
	return (
		<div className={styles.section}>
			{description ? <p className={styles.sectionDescription}>{description}</p> : null}
			{children}
		</div>
	);
}
