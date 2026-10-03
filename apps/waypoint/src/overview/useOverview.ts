// Gathers what Overview shows: the volumes and what can be done to them, the Trash, and where Home is
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { featureMessage, hasFeature, type Volume } from '@liminal-hq/plugin-volumes';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { Places } from '@liminal-hq/waypoint-protocol/generated/Places';
import type { TrashInfo } from '@liminal-hq/waypoint-protocol/generated/TrashInfo';
import type { VolumeSpace } from '@liminal-hq/waypoint-protocol/generated/VolumeSpace';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useDevices, type UnlockResult } from '../devices/useDevices';
import type { DevicesClient } from '../devices/devicesClient';
import { t, tf } from '../i18n/messages';
import type { VfsClient } from '../services/vfsClient';
import type { TrashClient } from '../trash/trashClient';
import { useTrashInfo } from '../trash/useTrashInfo';
import type { HomeMeasure } from './homeMeasure';
import { cardsOf, statsOf, type Stats, type VolumeCardModel } from './overviewModel';

/** Why the volume list is not shown, and what Overview shows in its place. */
export type VolumesState =
	/** Waiting for the volumes plugin's first answer. */
	| { kind: 'loading' }
	| { kind: 'ready' }
	/** The plugin cannot list volumes here (or the window has none): the sentence says why. */
	| { kind: 'unavailable'; reason: string };

export interface Overview {
	volumes: VolumesState;
	/** One card per volume; empty until the list arrives. */
	cards: readonly VolumeCardModel[];
	/** The card of the volume that holds Home when the plugin cannot list volumes: its space as the file system reports it. */
	fallback: VolumeSpace | null;
	stats: Stats;
	/** The Trash's state, or `null` until it is read (and without a Trash service). */
	trash: TrashInfo | null;
	/** The ids of the volumes being measured or worked on. */
	busy: ReadonlySet<string>;
	/** Whether the volumes plugin offers each action here. */
	canUnlock: boolean;
	canMount: boolean;
	/** Measures a network volume on request. */
	measure(volume: Volume): Promise<void>;
	mount(volume: Volume): Promise<boolean>;
	unlock(volume: Volume, passphrase: string): Promise<UnlockResult>;
}

export interface OverviewSources {
	devices: DevicesClient | null;
	trash: TrashClient | null;
	vfs: VfsClient;
	/** The sidebar's places, which say where Home is. */
	places: Places | null;
	homeMeasure: HomeMeasure;
	/** Says something politely to a screen reader. */
	announce(message: string): void;
	/** Reports something that could not be done. */
	notice(message: string): void;
}

/** Home's own folder, from the sidebar's places. */
export function homeOf(places: Places | null): Location | null {
	return places?.places.find((place) => place.kind === 'home')?.location ?? null;
}

/**
 * The volume cards are drawn as soon as the volumes plugin has listed them: the Trash's size, a
 * network volume's space and the size of Home each arrive on their own and never hold the cards
 * back. While Overview is on screen the Trash is read with its total size, which the sidebar never
 * asks for (it lists the Trash, so it is not worth doing off screen).
 *
 * A network volume is measured only when the person asks (`measure`), because reaching a share
 * can take as long as the plugin's timeout. When the plugin cannot list volumes the reason is
 * reported and the volume that holds Home is shown from the file system's own numbers.
 */
export function useOverview(sources: OverviewSources): Overview {
	const {
		devices: client,
		trash: trashClient,
		vfs,
		places,
		homeMeasure,
		announce,
		notice,
	} = sources;
	const gate = useRef(false);
	// The device hooks announce plugged and unplugged drives into the sidebar's own live region; this
	// page speaks only about what it was asked to do.
	const announceAsked = useCallback(
		(message: string) => {
			if (gate.current) announce(message);
		},
		[announce],
	);
	const devices = useDevices(
		client,
		useMemo(() => ({ announce: announceAsked, notice }), [announceAsked, notice]),
	);
	const trash = useTrashInfo(trashClient, { withBytes: true });
	const home = homeOf(places);

	// What measuring a network volume found, by volume id.
	const [measured, setMeasured] = useState<ReadonlyMap<string, Volume>>(new Map());
	const [measuring, setMeasuring] = useState<ReadonlySet<string>>(new Set());
	const merged = useMemo(
		() =>
			devices.volumes.map((volume) => {
				const found = measured.get(volume.id);
				return found && volume.free === null
					? { ...volume, total: found.total, free: found.free }
					: volume;
			}),
		[devices.volumes, measured],
	);
	const cards = useMemo(() => cardsOf(merged, home?.display ?? null), [merged, home?.display]);
	const stats = useMemo(() => statsOf(cards, homeMeasure), [cards, homeMeasure]);

	const measure = useCallback(
		async (volume: Volume) => {
			if (!client) return;
			setMeasuring((have) => new Set(have).add(volume.id));
			try {
				const found = await client.refreshSpace(volume.id);
				if (found.total === null || found.free === null) {
					notice(tf('overview.volume.measureFailed', { name: volume.label }));
					announce(tf('overview.volume.measureFailed', { name: volume.label }));
				} else {
					setMeasured((have) => new Map(have).set(volume.id, found));
					announce(tf('overview.announce.measured', { name: volume.label }));
				}
			} catch (error) {
				console.warn('could not measure a volume', error);
				notice(tf('overview.volume.measureFailed', { name: volume.label }));
				announce(tf('overview.volume.measureFailed', { name: volume.label }));
			} finally {
				setMeasuring((have) => {
					const next = new Set(have);
					next.delete(volume.id);
					return next;
				});
			}
		},
		[client, notice, announce],
	);

	const { run, unlock: unlockVolume } = devices;
	const mount = useCallback(
		async (volume: Volume) => {
			gate.current = true;
			try {
				return await run('mount', volume);
			} finally {
				gate.current = false;
			}
		},
		[run],
	);
	const unlock = useCallback(
		async (volume: Volume, passphrase: string) => {
			gate.current = true;
			try {
				return await unlockVolume(volume, passphrase);
			} finally {
				gate.current = false;
			}
		},
		[unlockVolume],
	);

	// With the volume list unavailable, the volume that holds Home is still worth showing.
	const status = devices.status;
	const unavailable = client === null || (status !== null && !devices.available);
	const [fallback, setFallback] = useState<VolumeSpace | null>(null);
	const homeUri = home?.uri;
	useEffect(() => {
		if (!unavailable || !home) return;
		let live = true;
		vfs.getFreeSpace(home).then(
			(space) => {
				if (live) setFallback(space);
			},
			() => {
				if (live) setFallback(null);
			},
		);
		return () => {
			live = false;
		};
	}, [unavailable, vfs, homeUri]);

	let volumes: VolumesState;
	if (unavailable) {
		const reason = status
			? (featureMessage(status, 'list') ?? status.message ?? t('overview.unavailable.fallback'))
			: t('overview.unavailable.fallback');
		volumes = { kind: 'unavailable', reason };
	} else if (status === null) {
		volumes = { kind: 'loading' };
	} else {
		volumes = { kind: 'ready' };
	}

	const busy = useMemo(() => new Set([...devices.busy, ...measuring]), [devices.busy, measuring]);
	return {
		volumes,
		cards: unavailable ? [] : cards,
		fallback: unavailable ? fallback : null,
		stats,
		trash,
		busy,
		canUnlock: status !== null && hasFeature(status, 'unlock'),
		canMount: status !== null && hasFeature(status, 'mount'),
		measure,
		mount,
		unlock,
	};
}
