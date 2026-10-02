// The Transparency page's live preview: a sample window over a busy wallpaper, drawn with the page's own opacity rules
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { CSSProperties } from 'react';
import { t } from '../i18n/messages';
import type { EffectiveTransparency } from '../theme/transparency';
import styles from './TransparencyPreview.module.css';

interface TransparencyPreviewProps {
	/** What each part of the window would draw, from the same function the real windows use. */
	effective: EffectiveTransparency;
}

/** A solid theme colour at `alpha`, the way `tokens.css` mixes a translucent region. */
function tint(token: string, alpha: number): string {
	return `color-mix(in srgb, var(${token}) ${Math.round(alpha * 100)}%, transparent)`;
}

/**
 * Draws a sample window over a wallpaper with bright bands, so what shows through is easy to see,
 * using the opacity of each part that the real windows use (the contrast floor included). It is
 * drawn from the values it is given, so it follows the slider as it is dragged, before anything
 * is saved. It is a picture: its contents are hidden from assistive technology except for one label.
 */
export function TransparencyPreview({ effective }: TransparencyPreviewProps) {
	const { alphas } = effective;
	const part = (token: string, alpha: number): CSSProperties => ({
		background: tint(token, alpha),
	});
	return (
		<figure
			className={styles.preview}
			role="img"
			aria-label={t('settings.transparency.preview.label')}
		>
			<div className={styles.wallpaper} aria-hidden="true">
				<div className={styles.window} data-region="window">
					<div
						className={styles.titleBar}
						style={part('--wp-bg-raised', alphas.titleBar)}
						data-region="titleBar"
						data-alpha={alphas.titleBar}
					>
						<span className={styles.dot} />
						<span className={styles.dot} />
						<span className={styles.dot} />
						<span className={styles.titleText}>{t('settings.transparency.preview.title')}</span>
					</div>
					<div className={styles.body}>
						<div
							className={styles.sidebar}
							style={part('--wp-solid-sidebar', alphas.sidebar)}
							data-region="sidebar"
							data-alpha={alphas.sidebar}
						>
							<span className={styles.heading}>{t('settings.transparency.preview.sidebar')}</span>
							<span className={styles.item} />
							<span className={styles.item} />
							<span className={styles.item} />
						</div>
						<div className={styles.main}>
							<div
								className={styles.toolbar}
								style={part('--wp-solid-window', alphas.rows)}
								data-region="rows"
								data-alpha={alphas.rows}
							/>
							<div
								className={styles.content}
								style={part('--wp-solid-content', alphas.content)}
								data-region="content"
								data-alpha={alphas.content}
							>
								<span className={styles.row}>{t('settings.transparency.preview.file')}</span>
								<span className={styles.row}>{t('settings.transparency.preview.file')}</span>
								<span className={styles.row}>{t('settings.transparency.preview.file')}</span>
							</div>
						</div>
					</div>
					<ul
						className={styles.menu}
						style={{
							...part('--wp-bg-raised', alphas.menu),
							backdropFilter: alphas.menu < 1 ? 'blur(12px)' : undefined,
						}}
						data-region="menu"
						data-alpha={alphas.menu}
					>
						<li>{t('settings.transparency.preview.menu.open')}</li>
						<li>{t('settings.transparency.preview.menu.rename')}</li>
						<li>{t('settings.transparency.preview.menu.trash')}</li>
					</ul>
				</div>
			</div>
			<figcaption className={styles.caption}>
				{t('settings.transparency.preview.caption')}
			</figcaption>
		</figure>
	);
}
