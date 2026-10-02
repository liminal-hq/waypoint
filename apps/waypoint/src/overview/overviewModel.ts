// What Overview shows of the volumes: which count, the headline totals, each card's state and the words for a bar
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Volume } from '@liminal-hq/plugin-volumes';
import { formatSize } from '../browse/format';
import { visibleVolumes, ALMOST_FULL_PERCENT } from '../devices/deviceModel';
import { t, tf } from '../i18n/messages';
import type { HomeMeasure } from './homeMeasure';

/** The word a card's badge carries; "System" and "Removable" are the two a person mostly sees. */
export type VolumeBadge = 'system' | 'removable' | 'network' | 'optical';

/** What a card can say about a volume's space. */
export type SpaceState =
	/** Size and free space are known: the exact Used / Free bar. */
	| { kind: 'measured'; total: number; free: number; used: number; percent: number }
	/** A locked encrypted volume: nothing to measure until it is unlocked. */
	| { kind: 'locked' }
	/** Not mounted, so there is no free space to read. */
	| { kind: 'notMounted' }
	/** A network volume nobody has asked to measure (it can be slow). */
	| { kind: 'unmeasured' }
	/** Mounted, and the space could not be read (it did not answer in time, or is not supported). */
	| { kind: 'unavailable' };

/** One volume's card. */
export interface VolumeCardModel {
	volume: Volume;
	badge: VolumeBadge;
	/** The file system as the system names it, or `null` when it does not say. */
	fileSystem: string | null;
	space: SpaceState;
	/** Whether this is the volume that holds the home folder, which may show "Your files" within Used. */
	holdsHome: boolean;
}

/** The volumes that get a card: what is mounted, locked, or can be mounted, without disk images. */
export function cardVolumes(volumes: readonly Volume[]): Volume[] {
	return visibleVolumes(volumes).filter((volume) => volume.kind !== 'loop');
}

export function badgeOf(volume: Volume): VolumeBadge {
	if (volume.kind === 'network') return 'network';
	if (volume.kind === 'optical') return 'optical';
	return volume.isSystem || volume.kind === 'internal' ? 'system' : 'removable';
}

/** Whether a volume counts towards Capacity and Free: a local, mounted volume that reported its size and free space. */
export function isCounted(volume: Volume): boolean {
	return (
		volume.kind !== 'network' &&
		volume.kind !== 'loop' &&
		volume.mountPoint !== null &&
		volume.total !== null &&
		volume.total > 0 &&
		volume.free !== null
	);
}

export function spaceOf(volume: Volume): SpaceState {
	if (volume.locked) return { kind: 'locked' };
	if (volume.mountPoint === null) return { kind: 'notMounted' };
	const { total, free } = volume;
	if (total !== null && free !== null && total > 0) {
		const used = Math.min(total, Math.max(0, total - free));
		return {
			kind: 'measured',
			total,
			free: total - used,
			used,
			percent: Math.round((used / total) * 100),
		};
	}
	return { kind: volume.kind === 'network' ? 'unmeasured' : 'unavailable' };
}

/** Whether a bar is "almost full" in words and in its pattern, not only its colour. */
export function isAlmostFull(space: SpaceState): boolean {
	return space.kind === 'measured' && space.percent >= ALMOST_FULL_PERCENT;
}

// --- Finding the volume that holds Home ---------------------------------------------------------

function normalise(path: string, windows: boolean): string {
	let text = windows ? path.replace(/\\/g, '/').toLowerCase() : path;
	while (text.length > 1 && text.endsWith('/')) text = text.slice(0, -1);
	return text;
}

/**
 * The id of the mounted volume that holds `homePath`: the one with the longest mount point that
 * contains it. `null` when none does (or Home is not known yet).
 */
export function homeVolumeId(volumes: readonly Volume[], homePath: string | null): string | null {
	if (!homePath) return null;
	const windows = /^[a-z]:[\\/]/i.test(homePath) || homePath.startsWith('\\\\');
	const home = normalise(homePath, windows);
	let best: { id: string; length: number } | null = null;
	for (const volume of volumes) {
		if (volume.mountPoint === null || volume.kind === 'loop' || volume.kind === 'network') continue;
		const mount = normalise(volume.mountPoint, windows);
		const contains =
			mount === '/' || mount.endsWith(':/')
				? home.startsWith(mount)
				: home === mount || home.startsWith(`${mount}/`);
		if (contains && (best === null || mount.length > best.length)) {
			best = { id: volume.id, length: mount.length };
		}
	}
	return best?.id ?? null;
}

export function cardsOf(volumes: readonly Volume[], homePath: string | null): VolumeCardModel[] {
	const shown = cardVolumes(volumes);
	const home = homeVolumeId(shown, homePath);
	return shown.map((volume) => ({
		volume,
		badge: badgeOf(volume),
		fileSystem: volume.fileSystem && volume.fileSystem !== '' ? volume.fileSystem : null,
		space: spaceOf(volume),
		holdsHome: volume.id === home,
	}));
}

// --- The headline stats -------------------------------------------------------------------------

/** The four headline stats, as numbers; the view words them. */
export interface Stats {
	/** Sum of the sizes of the counted volumes; `null` when none reported one. */
	capacity: number | null;
	free: number | null;
	/** How many volumes the two sums count. */
	counted: number;
	home:
		| { status: 'notMeasured' }
		| { status: 'measuring' }
		| {
				status: 'done';
				bytes: number;
				/** Whole percent of the used space on the volume that holds Home, or `null` when that is not known. */
				sharePercent: number | null;
				volume: string | null;
		  };
	/** How many volumes have a card. */
	volumeCount: number;
	/** The file systems of those volumes, each once, in the order first seen. */
	fileSystems: string[];
}

export function statsOf(cards: readonly VolumeCardModel[], homeMeasure: HomeMeasure): Stats {
	const counted = cards.filter((card) => isCounted(card.volume));
	const capacity = counted.length === 0 ? null : sum(counted.map((card) => card.volume.total ?? 0));
	const free = counted.length === 0 ? null : sum(counted.map((card) => card.volume.free ?? 0));
	const fileSystems: string[] = [];
	for (const card of cards) {
		if (card.fileSystem && !fileSystems.includes(card.fileSystem))
			fileSystems.push(card.fileSystem);
	}
	let home: Stats['home'];
	if (homeMeasure.status !== 'done') {
		home = { status: homeMeasure.status };
	} else {
		const holder = cards.find((card) => card.holdsHome);
		const used = holder?.space.kind === 'measured' ? holder.space.used : 0;
		home = {
			status: 'done',
			bytes: homeMeasure.bytes,
			sharePercent: used > 0 ? Math.min(100, Math.round((homeMeasure.bytes / used) * 100)) : null,
			volume: holder?.volume.label ?? null,
		};
	}
	return { capacity, free, counted: counted.length, home, volumeCount: cards.length, fileSystems };
}

function sum(values: readonly number[]): number {
	return values.reduce((total, value) => total + value, 0);
}

// --- Bars ---------------------------------------------------------------------------------------

/** One part of a usage bar. */
export interface BarSegment {
	key: 'files' | 'other' | 'used' | 'free';
	bytes: number;
	/** Whole percent of the volume's size. */
	percent: number;
}

/** What a bar draws and what its legend and text equivalent say. */
export interface BarModel {
	segments: BarSegment[];
	almostFull: boolean;
	/** "Backup: 125 GB used and 375 GB free of 500 GB (25% used)", the text equivalent of the bar. */
	text: string;
	/** The same numbers one by one, for the legend. */
	legend: Array<{ key: BarSegment['key']; bytes: number; percent: number }>;
}

/**
 * The exact Used / Free bar of a measured card. `files` is the size of the home folder when this is
 * the volume that holds it and it has been measured: it splits Used into "Your files" and
 * "Everything else", never more than Used itself (the home folder is measured by its apparent
 * size and may not be on this volume's used count to the byte).
 */
export function barOf(
	name: string,
	space: Extract<SpaceState, { kind: 'measured' }>,
	files: number | null,
): BarModel {
	const { total, used, free } = space;
	const percentOf = (bytes: number) => (total > 0 ? Math.round((bytes / total) * 100) : 0);
	const almostFull = isAlmostFull(space);
	const yours = files === null ? null : Math.min(used, Math.max(0, files));
	const segments: BarSegment[] =
		yours === null
			? [{ key: 'used', bytes: used, percent: percentOf(used) }]
			: [
					{ key: 'files', bytes: yours, percent: percentOf(yours) },
					{ key: 'other', bytes: used - yours, percent: percentOf(used - yours) },
				];
	segments.push({ key: 'free', bytes: free, percent: percentOf(free) });
	const values = {
		name,
		used: formatSize(used),
		free: formatSize(free),
		total: formatSize(total),
		percent: space.percent,
	};
	const text =
		yours === null
			? tf(almostFull ? 'overview.bar.labelAlmostFull' : 'overview.bar.label', values)
			: tf('overview.bar.labelHome', {
					...values,
					files: formatSize(yours),
					other: formatSize(used - yours),
				});
	return {
		segments,
		almostFull,
		text: yours !== null && almostFull ? `${text}. ${t('overview.volume.almostFull')}` : text,
		legend: segments.map(({ key, bytes, percent }) => ({ key, bytes, percent })),
	};
}
