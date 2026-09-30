// Placeholder screen shared by every window kind until its real UI lands
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { WindowFrame } from '@liminal-hq/waypoint-chrome/WindowFrame';
import { AppTitleBar } from './AppTitleBar';
import styles from './PlaceholderScreen.module.css';

interface PlaceholderScreenProps {
	title: string;
	description: string;
}

export function PlaceholderScreen({ title, description }: PlaceholderScreenProps) {
	return (
		<WindowFrame className={styles.screen}>
			<AppTitleBar title={title} />
			<main className={styles.content}>
				<h1 className={styles.heading}>{title}</h1>
				<p className={styles.description}>{description}</p>
			</main>
		</WindowFrame>
	);
}
