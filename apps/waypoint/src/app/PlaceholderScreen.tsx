// Placeholder screen shared by every window kind until its real UI lands
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { TitleBar } from '@liminal-hq/waypoint-chrome/TitleBar';
import { WindowFrame } from '@liminal-hq/waypoint-chrome/WindowFrame';
import { tauriWindowControls } from '@liminal-hq/waypoint-chrome/TitleBar/tauriWindowControls';
import styles from './PlaceholderScreen.module.css';

interface PlaceholderScreenProps {
	title: string;
	description: string;
}

export function PlaceholderScreen({ title, description }: PlaceholderScreenProps) {
	return (
		<WindowFrame windowControls={tauriWindowControls} className={styles.screen}>
			<TitleBar
				windowControls={tauriWindowControls}
				center={
					// Tauri's drag handler only checks the pressed element, not its ancestors, so
					// static text inside a slot needs the attribute itself.
					<span className={styles.title} data-tauri-drag-region="">
						{title}
					</span>
				}
				showAlwaysOnTop
				transparent
			/>
			<main className={styles.content}>
				<h1 className={styles.heading}>{title}</h1>
				<p className={styles.description}>{description}</p>
			</main>
		</WindowFrame>
	);
}
