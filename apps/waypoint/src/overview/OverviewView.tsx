// Overview: where the space went and what is plugged in, as the content of a tab
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Volume } from '@liminal-hq/plugin-volumes';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { useCallback, useId, useMemo, useState } from 'react';
import { useVfsClient } from '../browse/VfsClientContext';
import { formatSize } from '../browse/format';
import { useDevicesClient } from '../devices/DevicesClientContext';
import { UnlockDialog } from '../devices/UnlockDialog';
import { t, tf, tn } from '../i18n/messages';
import { useNavigation } from '../nav/useNavigation';
import { usePlacesClient } from '../sidebar/PlacesClientContext';
import { usePlaces } from '../sidebar/usePlaces';
import { showNotice } from '../app/notices';
import { useTrashClient } from '../trash/TrashClientContext';
import { useTrashActions } from '../trash/trashJobs';
import { TRASH_LOCATION } from '../trash/trashLocation';
import { useHourCycle } from '../browse/TimeFormatContext';
import { formatModified } from '../browse/format';
import { useHomeMeasure, type HomeMeasure } from './homeMeasure';
import { HomeUsage } from './HomeUsage';
import type { Stats, VolumeCardModel } from './overviewModel';
import { TrashCard } from './TrashCard';
import { homeOf, useOverview } from './useOverview';
import { VolumeCard } from './VolumeCard';
import styles from './OverviewView.module.css';

interface OverviewViewProps {
	/** The tab this page is the content of: opening a volume or the Trash moves that tab. */
	tabId: number;
}

/**
 * The page paints its volume cards as soon as the volumes plugin has listed them; the Trash's
 * size, a network volume's space and the size of Home each arrive on their own. It speaks politely
 * through its own live region and reports a failure inline and in the status bar. Everything
 * on it is reachable by keyboard in reading order: the headline stats, then each card's open
 * button and its actions, then the Trash's.
 */
export function OverviewView({ tabId }: OverviewViewProps) {
	const titleId = useId();
	const [announcement, setAnnouncement] = useState('');
	const places = usePlaces(usePlacesClient());
	const home = homeOf(places);
	const usage = useHomeMeasure(home);
	const homeNow = usage.measure;
	const hourCycle = useHourCycle();
	const trashClient = useTrashClient();
	const overview = useOverview({
		devices: useDevicesClient(),
		trash: trashClient,
		vfs: useVfsClient(),
		places,
		homeMeasure: homeNow,
		announce: setAnnouncement,
		notice: showNotice,
	});
	const navigation = useNavigation(tabId);
	const trashActions = useTrashActions();
	const vfs = useVfsClient();
	const [unlocking, setUnlocking] = useState<Volume | null>(null);
	const { goTo, tab } = navigation;
	const here = tab?.location;

	const open = useCallback(
		(card: VolumeCardModel) => {
			const mountPoint = card.volume.mountPoint;
			if (!mountPoint || !here) return;
			vfs.parseLocation(mountPoint, here).then(goTo, (error: unknown) => {
				console.warn('could not open a volume', error);
				const message = tf('devices.open.failed', { name: card.volume.label });
				showNotice(message);
				setAnnouncement(message);
			});
		},
		[vfs, here, goTo],
	);

	const asOf =
		homeNow.status === 'done' && homeNow.asOfMs !== null
			? formatModified(homeNow.asOfMs, undefined, hourCycle)
			: null;
	const stats = useMemo(
		() => statTiles(overview.stats, homeNow, asOf),
		[overview.stats, homeNow, asOf],
	);
	const openFolder = useCallback(
		(location: Location) => {
			goTo(location).catch((error: unknown) => console.warn('could not open a folder', error));
		},
		[goTo],
	);

	return (
		<div className={styles.page} role="region" aria-labelledby={titleId} tabIndex={-1}>
			<h2 id={titleId} className={styles.title}>
				{t('overview.title')}
			</h2>

			<dl className={styles.stats} aria-label={t('overview.stats.label')}>
				{stats.map((tile) => (
					<div key={tile.id} className={styles.stat} data-stat={tile.id}>
						<dt>{tile.label}</dt>
						<dd className={styles.statValue}>{tile.value}</dd>
						<dd className={styles.statNote}>{tile.note}</dd>
					</div>
				))}
			</dl>

			<section className={styles.section} aria-label={t('overview.volumes.label')}>
				{overview.volumes.kind === 'unavailable' && (
					<div className={styles.unavailable} role="note">
						<strong>{t('overview.unavailable.title')}</strong>
						<p>{overview.volumes.reason}</p>
						{overview.fallback && <p>{t('overview.unavailable.showing')}</p>}
					</div>
				)}
				{overview.volumes.kind === 'loading' && (
					<p className={styles.quiet}>{t('overview.loading')}</p>
				)}
				{overview.volumes.kind === 'ready' && overview.cards.length === 0 && (
					<p className={styles.quiet}>{t('overview.volumes.empty')}</p>
				)}
				<div className={styles.cards}>
					{overview.cards.map((card) => (
						<VolumeCard
							key={card.volume.id}
							card={card}
							homeMeasure={homeNow}
							busy={overview.busy.has(card.volume.id)}
							canMount={overview.canMount}
							canUnlock={overview.canUnlock}
							onOpen={open}
							onMeasure={(target) => void overview.measure(target.volume)}
							onMount={(target) => void overview.mount(target.volume)}
							onUnlock={(target) => setUnlocking(target.volume)}
						/>
					))}
					{overview.fallback && (
						<VolumeCard
							card={fallbackCard(overview.fallback)}
							homeMeasure={homeNow}
							busy={false}
							canMount={false}
							canUnlock={false}
							onOpen={() => {}}
							onMeasure={() => {}}
							onMount={() => {}}
							onUnlock={() => {}}
						/>
					)}
					{trashClient !== null && (
						<TrashCard
							info={overview.trash}
							canEmpty={trashActions !== null}
							onOpen={() => void goTo(TRASH_LOCATION)}
							onEmpty={(count) => trashActions?.emptyTrash(count)}
						/>
					)}
				</div>
			</section>

			{usage.available && (
				<HomeUsage usage={usage} asOf={asOf} onOpen={openFolder} announce={setAnnouncement} />
			)}

			{unlocking && (
				<UnlockDialog
					volume={unlocking}
					onUnlock={overview.unlock}
					onClose={() => setUnlocking(null)}
				/>
			)}
			<div role="status" className={styles.srOnly}>
				{announcement}
			</div>
		</div>
	);
}

interface StatTile {
	id: 'capacity' | 'free' | 'home' | 'volumes';
	label: string;
	value: string;
	note: string;
}

/** The four headline stats, worded. Home reads "Not measured yet" until the directory-size scan provides it. */
export function statTiles(
	stats: Stats,
	homeMeasure: HomeMeasure,
	asOf: string | null = null,
): StatTile[] {
	const capacity: StatTile = {
		id: 'capacity',
		label: t('overview.stat.capacity'),
		value: stats.capacity === null ? t('overview.volume.unavailable') : formatSize(stats.capacity),
		note:
			stats.capacity === null
				? t('overview.stat.capacity.none')
				: tn('overview.stat.capacity.note', stats.counted),
	};
	const free: StatTile = {
		id: 'free',
		label: t('overview.stat.free'),
		value: stats.free === null ? t('overview.volume.unavailable') : formatSize(stats.free),
		note: t('overview.stat.free.note'),
	};
	let home: StatTile;
	if (stats.home.status === 'done') {
		const share =
			stats.home.sharePercent === null || stats.home.volume === null
				? t('overview.stat.home.noteUnknown')
				: tf('overview.stat.home.note', {
						percent: stats.home.sharePercent,
						volume: stats.home.volume,
					});
		home = {
			id: 'home',
			label: t('overview.stat.home'),
			value: formatSize(stats.home.bytes),
			note: asOf === null ? share : `${share} · ${tf('overview.home.asOf', { time: asOf })}`,
		};
	} else {
		home = {
			id: 'home',
			label: t('overview.stat.home'),
			value:
				homeMeasure.status === 'measuring'
					? t('overview.stat.home.measuring')
					: t('overview.stat.home.notMeasured'),
			note: t('overview.stat.home.noteUnknown'),
		};
	}
	const volumes: StatTile = {
		id: 'volumes',
		label: t('overview.stat.volumes'),
		value: new Intl.NumberFormat().format(stats.volumeCount),
		note:
			stats.fileSystems.length === 0
				? t('overview.stat.volumes.noSystems')
				: stats.fileSystems.join(', '),
	};
	return [capacity, free, home, volumes];
}

/** The card of the volume that holds Home when the volumes plugin cannot list volumes: only what the file system itself says. */
function fallbackCard(space: { totalBytes: number; freeBytes: number }): VolumeCardModel {
	const used = Math.max(0, space.totalBytes - space.freeBytes);
	const volume: Volume = {
		id: 'home-volume',
		label: t('overview.volume.homeVolume'),
		kind: 'internal',
		fileSystem: null,
		mountPoint: null,
		uri: null,
		total: space.totalBytes,
		free: space.freeBytes,
		canMount: false,
		canUnmount: false,
		canEject: false,
		canPowerOff: false,
		locked: false,
		isSystem: true,
		device: null,
	};
	return {
		volume,
		badge: 'system',
		fileSystem: null,
		space: {
			kind: 'measured',
			total: space.totalBytes,
			free: space.freeBytes,
			used,
			percent: space.totalBytes > 0 ? Math.round((used / space.totalBytes) * 100) : 0,
		},
		holdsHome: true,
	};
}
