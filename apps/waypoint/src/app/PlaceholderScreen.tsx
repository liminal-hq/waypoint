// Placeholder screen shared by every window kind until its real UI lands
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import styles from './PlaceholderScreen.module.css';

interface PlaceholderScreenProps {
	title: string;
	description: string;
}

export function PlaceholderScreen({ title, description }: PlaceholderScreenProps) {
	return (
		<div className={styles.screen}>
			<header className={styles.header}>
				<h1 className={styles.title}>{title}</h1>
			</header>
			<main className={styles.content}>
				<p className={styles.description}>{description}</p>
			</main>
		</div>
	);
}
