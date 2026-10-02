// Titled group of settings rows, and the section wrapper that holds groups
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useId, type ReactNode } from 'react';
import '../tokens.css';
import styles from './SettingsGroup.module.css';

export interface SettingsGroupProps {
	/** Group heading. Without one the group has no visible title and no accessible name. */
	title?: string;
	description?: ReactNode;
	children: ReactNode;
}

export function SettingsGroup({ title, description, children }: SettingsGroupProps) {
	const id = useId();
	return (
		<section className={styles.group} role="group" aria-labelledby={title ? id : undefined}>
			{title ? (
				<h3 id={id} className={styles.groupTitle}>
					{title}
				</h3>
			) : null}
			{description ? <p className={styles.groupDescription}>{description}</p> : null}
			<div className={styles.rows}>{children}</div>
		</section>
	);
}
