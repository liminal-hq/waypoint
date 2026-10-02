// The Trash's card: how many items it holds, their size when known, and Open and Empty
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { TrashInfo } from '@liminal-hq/waypoint-protocol/generated/TrashInfo';
import { useId } from 'react';
import { formatSize } from '../browse/format';
import { t, tn } from '../i18n/messages';
import { TrashIcon } from '../icons/MenuIcons';
import styles from './VolumeCard.module.css';

interface TrashCardProps {
	/** The Trash's state, or `null` while it is being read. */
	info: TrashInfo | null;
	/** Whether Empty can be offered: the window has the Trash's jobs. */
	canEmpty: boolean;
	onOpen(): void;
	/** Asks, with the Trash's own confirmation, then empties it. */
	onEmpty(count: number): void;
}

/**
 * The count is always there once the Trash has been read; the size is added only when the plugin
 * could add it up, and "Size not measured" says so otherwise (a missing size is never shown as
 * zero). A Trash that cannot be browsed here, such as the sandbox's portal that can only trash,
 * says why in the plugin's own words and offers neither action. Empty goes through the same
 * confirmation as everywhere else.
 */
export function TrashCard({ info, canEmpty, onOpen, onEmpty }: TrashCardProps) {
	const titleId = useId();
	return (
		<section className={styles.card} role="group" aria-labelledby={titleId} data-trash="">
			<header className={styles.header}>
				<TrashIcon className={styles.icon} />
				<h3 id={titleId} className={styles.name}>
					{t('overview.trash.title')}
				</h3>
			</header>
			{info === null && <p className={styles.state}>{t('overview.trash.reading')}</p>}
			{info !== null && !info.available && (
				<>
					<p className={styles.state}>{t('overview.trash.unavailable')}</p>
					<p className={styles.system}>{info.reason ?? t('trash.unavailable.fallback')}</p>
				</>
			)}
			{info?.available && (
				<>
					<dl className={styles.numbers}>
						<div>
							<dt>{t('overview.trash.items')}</dt>
							<dd>
								{info.count === 0
									? t('overview.trash.empty')
									: tn('overview.trash.count', info.count)}
							</dd>
						</div>
						{info.count > 0 && (
							<div>
								<dt>{t('overview.volume.size')}</dt>
								<dd>
									{info.totalBytes === null
										? t('overview.trash.sizeUnknown')
										: formatSize(info.totalBytes)}
								</dd>
							</div>
						)}
					</dl>
					<p className={styles.system}>{t('overview.trash.unverified')}</p>
					<div className={styles.actions}>
						<button type="button" className={styles.action} onClick={onOpen}>
							{t('overview.trash.open')}
						</button>
						{canEmpty && (
							<button
								type="button"
								className={styles.action}
								disabled={info.count === 0}
								onClick={() => onEmpty(info.count)}
							>
								{t('overview.trash.emptyAction')}
							</button>
						)}
					</div>
				</>
			)}
		</section>
	);
}
