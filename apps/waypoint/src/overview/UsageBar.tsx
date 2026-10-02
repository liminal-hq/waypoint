// A volume's usage bar: segments that differ by pattern as well as colour, a numeric legend and a text equivalent
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { formatSize } from '../browse/format';
import { t, tf, type MessageId } from '../i18n/messages';
import type { BarModel, BarSegment } from './overviewModel';
import styles from './UsageBar.module.css';

const LEGEND: Record<BarSegment['key'], MessageId> = {
	files: 'overview.bar.legend.files',
	other: 'overview.bar.legend.other',
	used: 'overview.bar.legend.used',
	free: 'overview.bar.legend.free',
};

interface UsageBarProps {
	bar: BarModel;
	/** Said by the bar to a screen reader before its numbers: who the bar belongs to. */
	name: string;
	/** Shown under the legend where the bar has a part it cannot draw yet ("Your files: not measured yet"). */
	pending?: string | undefined;
}

/**
 * The bar is one image to a screen reader, whose label is the text equivalent ("Backup: 125 GB
 * used and 375 GB free of 500 GB (25% used)"); the legend repeats the same numbers as text for
 * everyone, and each part has its own pattern (solid, diagonal lines, dots, hatching when nearly
 * full), so no colour is needed to tell them apart. A part narrower than a pixel is still drawn at
 * a minimum width so it is never lost.
 */
export function UsageBar({ bar, name, pending }: UsageBarProps) {
	return (
		<div className={styles.usage}>
			<div
				className={styles.track}
				role="img"
				aria-label={bar.text}
				data-full={bar.almostFull ? '' : undefined}
			>
				{bar.segments
					.filter((segment) => segment.key !== 'free' && segment.bytes > 0)
					.map((segment) => (
						<span
							key={segment.key}
							className={styles.segment}
							data-part={segment.key}
							style={{ inlineSize: `${Math.max(segment.percent, 0.5)}%` }}
						/>
					))}
			</div>
			<ul className={styles.legend} aria-label={tf('overview.bar.legend', { name })}>
				{bar.legend.map((item) => (
					<li key={item.key} className={styles.item}>
						<span className={styles.swatch} data-part={item.key} aria-hidden="true" />
						<span className={styles.term}>{t(LEGEND[item.key])}</span>
						<span className={styles.value}>
							{tf('overview.bar.value', { size: formatSize(item.bytes), percent: item.percent })}
						</span>
					</li>
				))}
				{pending && <li className={styles.pending}>{pending}</li>}
			</ul>
		</div>
	);
}
