// One volume's card: its name, file system and badge, its space, an exact Used / Free bar and the actions it allows
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useId } from 'react';
import { formatSize } from '../browse/format';
import { t, tf, type MessageId } from '../i18n/messages';
import { DriveIcon } from '../icons/MenuIcons';
import type { HomeMeasure } from './homeMeasure';
import { barOf, type VolumeBadge, type VolumeCardModel } from './overviewModel';
import { UsageBar } from './UsageBar';
import styles from './VolumeCard.module.css';

const BADGES: Record<VolumeBadge, MessageId> = {
	system: 'overview.volume.badge.system',
	removable: 'overview.volume.badge.removable',
	network: 'overview.volume.badge.network',
	optical: 'overview.volume.badge.optical',
};

interface VolumeCardProps {
	card: VolumeCardModel;
	homeMeasure: HomeMeasure;
	/** A measurement or an action is running on this volume. */
	busy: boolean;
	canMount: boolean;
	canUnlock: boolean;
	onOpen(card: VolumeCardModel): void;
	onMeasure(card: VolumeCardModel): void;
	onMount(card: VolumeCardModel): void;
	onUnlock(card: VolumeCardModel): void;
}

/**
 * The card is a group named by the volume. Its name is a button that opens the volume (a mounted
 * one), so the card is keyboard-activatable and read as "Open Backup"; Unlock, Mount and Measure
 * are buttons of their own and appear only where the volume allows them and the plugin reports
 * the feature as working. Everything the bar draws is also written: its text equivalent, the
 * legend under it, and the size and free space as plain text.
 */
export function VolumeCard({
	card,
	homeMeasure,
	busy,
	canMount,
	canUnlock,
	onOpen,
	onMeasure,
	onMount,
	onUnlock,
}: VolumeCardProps) {
	const { volume, space } = card;
	const titleId = useId();
	const name = volume.label;
	const files = card.holdsHome && homeMeasure.status === 'done' ? homeMeasure.bytes : null;
	const yourFilesPending =
		card.holdsHome && space.kind === 'measured' && homeMeasure.status !== 'done'
			? homeMeasure.status === 'measuring'
				? t('overview.volume.measuring')
				: t('overview.bar.legend.filesPending')
			: undefined;
	const mounted = volume.mountPoint !== null;
	return (
		<section
			className={styles.card}
			role="group"
			aria-labelledby={titleId}
			aria-busy={busy || undefined}
			data-volume={volume.id}
		>
			<header className={styles.header}>
				<DriveIcon className={styles.icon} />
				<h3 id={titleId} className={styles.name}>
					{mounted ? (
						<button
							type="button"
							className={styles.open}
							disabled={busy}
							title={volume.mountPoint ?? undefined}
							aria-label={tf('overview.volume.open', { name })}
							onClick={() => onOpen(card)}
						>
							{name}
						</button>
					) : (
						name
					)}
				</h3>
				<span className={styles.badge} data-badge={card.badge}>
					{t(BADGES[card.badge])}
				</span>
			</header>
			<p className={styles.system}>{card.fileSystem ?? t('overview.volume.unknownFileSystem')}</p>

			{space.kind === 'measured' && (
				<>
					<dl className={styles.numbers}>
						<div>
							<dt>{t('overview.volume.size')}</dt>
							<dd>{formatSize(space.total)}</dd>
						</div>
						<div>
							<dt>{t('overview.volume.free')}</dt>
							<dd>{formatSize(space.free)}</dd>
						</div>
					</dl>
					<UsageBar bar={barOf(name, space, files)} name={name} pending={yourFilesPending} />
				</>
			)}
			{space.kind === 'locked' && <p className={styles.state}>{t('overview.volume.locked')}</p>}
			{space.kind === 'notMounted' && (
				<p className={styles.state}>
					{volume.total !== null
						? `${t('overview.volume.notMounted')}, ${formatSize(volume.total)}`
						: t('overview.volume.notMounted')}
				</p>
			)}
			{space.kind === 'unmeasured' && (
				<p className={styles.state}>
					{busy ? t('overview.volume.measuring') : t('overview.volume.unmeasured')}
				</p>
			)}
			{space.kind === 'unavailable' && (
				<p className={styles.state}>{t('overview.volume.unavailable')}</p>
			)}

			<div className={styles.actions}>
				{space.kind === 'locked' && canUnlock && (
					<button
						type="button"
						className={styles.action}
						disabled={busy}
						aria-label={tf('overview.action.unlockVolume', { name })}
						onClick={() => onUnlock(card)}
					>
						{t('overview.action.unlock')}
					</button>
				)}
				{space.kind === 'notMounted' && volume.canMount && canMount && (
					<button
						type="button"
						className={styles.action}
						disabled={busy}
						aria-label={tf('overview.action.mountVolume', { name })}
						onClick={() => onMount(card)}
					>
						{t('overview.action.mount')}
					</button>
				)}
				{space.kind === 'unmeasured' && (
					<button
						type="button"
						className={styles.action}
						disabled={busy}
						aria-label={tf('overview.action.measureVolume', { name })}
						onClick={() => onMeasure(card)}
					>
						{t('overview.action.measure')}
					</button>
				)}
			</div>
		</section>
	);
}
