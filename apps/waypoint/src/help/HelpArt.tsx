// The picture at the top of Help: a dotted route between three places, with the app's mark travelling along it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t } from '../i18n/messages';
import { AppMarkIcon } from '../icons/AppIcons';
import styles from './HelpArt.module.css';
import { useReducedMotion } from './useReducedMotion';

/** The three places the route joins: its start, its middle and its end. */
const PLACES = [
	{ x: 14, y: 58, pulse: 'start' },
	{ x: 232, y: 44, pulse: 'middle' },
	{ x: 462, y: 52, pulse: 'end' },
] as const;

/**
 * Decoration only, so it is hidden from assistive technology. Under "reduce motion" nothing
 * moves: the route is drawn once with the mark resting in the middle of it.
 */
export function HelpArt() {
	const reduced = useReducedMotion();
	return (
		<div className={styles.art} aria-hidden="true" data-reduced-motion={reduced ? '' : undefined}>
			<div className={styles.stage}>
				<svg className={styles.route} viewBox="0 0 476 96" focusable="false">
					<path
						className={styles.path}
						d="M14 58 C 84 8, 150 96, 232 44 S 380 14, 462 52"
						fill="none"
					/>
					{PLACES.map((place) => (
						<circle
							key={place.pulse}
							className={`${styles.place} ${styles[place.pulse]}`}
							cx={place.x}
							cy={place.y}
							r="5"
							fill="none"
						/>
					))}
				</svg>
				<div className={styles.traveller}>
					<div className={styles.bob}>
						<AppMarkIcon width={22} height={22} />
					</div>
				</div>
			</div>
			<span className={styles.caption}>{t('help.art.caption')}</span>
		</div>
	);
}
