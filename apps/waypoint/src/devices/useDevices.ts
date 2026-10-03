// Keeps the Devices section current: the volume list, what the plugin can do, live changes, and the actions
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import {
	hasFeature,
	isVolumesError,
	type PluginStatus,
	type Volume,
} from '@liminal-hq/plugin-volumes';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { tf, type MessageId } from '../i18n/messages';
import { failureText, type DeviceAction } from './deviceModel';
import type { DevicesClient } from './devicesClient';

const DONE: Record<Exclude<DeviceAction, 'unlock'>, MessageId> = {
	mount: 'devices.announce.mounted',
	unmount: 'devices.announce.unmounted',
	eject: 'devices.announce.ejected',
};

export interface DevicesCallbacks {
	/** Says something politely to a screen reader. */
	announce(message: string): void;
	/** Reports something that could not be done, for the status bar. */
	notice(message: string): void;
}

/** What an unlock came to: the dialog stays open on `wrong`, closes on `unlocked` and `failed`. */
export type UnlockResult = 'unlocked' | 'wrong' | 'failed';

export interface Devices {
	/** The plugin's status, or `null` until it answers. */
	status: PluginStatus | null;
	/** True when the section should be shown: a client, and the plugin can list volumes. */
	available: boolean;
	volumes: readonly Volume[];
	/** The ids of the volumes an action is running on. */
	busy: ReadonlySet<string>;
	run(action: Exclude<DeviceAction, 'unlock'>, volume: Volume): Promise<boolean>;
	unlock(volume: Volume, passphrase: string): Promise<UnlockResult>;
}

/**
 * Reads the status and the list, then follows `volumes://changed`: an event whose revision is not
 * above the last one seen is ignored, and a list read that an event overtook is dropped. A volume
 * that appears or goes without the person asking is announced; one the person ejected is
 * announced by the eject instead. A failed action is announced and reported, with the reason (and
 * what is using the volume, when the plugin says) in words (D120).
 */
export function useDevices(client: DevicesClient | null, callbacks: DevicesCallbacks): Devices {
	const [status, setStatus] = useState<PluginStatus | null>(null);
	const [volumes, setVolumes] = useState<readonly Volume[]>([]);
	const [busy, setBusy] = useState<ReadonlySet<string>>(new Set());
	const known = useRef<ReadonlyMap<string, Volume> | null>(null);
	const quiet = useRef(new Set<string>());
	const running = useRef(0);
	const callbackRef = useRef(callbacks);
	callbackRef.current = callbacks;

	useEffect(() => {
		if (!client) return;
		let live = true;
		let seen = 0;
		let overtaken = false;
		const apply = (next: Volume[], announceChanges: boolean) => {
			const before = known.current;
			known.current = new Map(next.map((volume) => [volume.id, volume]));
			setVolumes(next);
			if (!before || !announceChanges) return;
			for (const volume of next) {
				if (!before.has(volume.id) && running.current === 0)
					callbackRef.current.announce(tf('devices.announce.added', { name: volume.label }));
			}
			for (const [id, volume] of before) {
				if (known.current.has(id)) continue;
				if (quiet.current.delete(id)) continue;
				callbackRef.current.announce(tf('devices.announce.removed', { name: volume.label }));
			}
		};
		const stop = client.onChanged((event) => {
			if (!live || event.revision <= seen) return;
			seen = event.revision;
			overtaken = true;
			apply(event.volumes, true);
		});
		client.getStatus().then(
			(next) => {
				if (live) setStatus(next);
				if (!live || !hasFeature(next, 'list')) return;
				client.list().then(
					(first) => {
						if (live && !overtaken) apply(first, false);
					},
					(error) => console.warn('could not list the volumes', error),
				);
			},
			(error) => console.warn('could not read the volumes status', error),
		);
		return () => {
			live = false;
			stop();
		};
	}, [client]);

	const setBusyFor = useCallback((id: string, on: boolean) => {
		setBusy((have) => {
			const next = new Set(have);
			if (on) next.add(id);
			else next.delete(id);
			return next;
		});
	}, []);

	const fail = useCallback((action: DeviceAction, volume: Volume, error: unknown) => {
		console.warn(`could not ${action} a volume`, error);
		const message = failureText(action, volume.label, error);
		callbackRef.current.announce(message);
		callbackRef.current.notice(message);
	}, []);

	const run = useCallback(
		async (action: Exclude<DeviceAction, 'unlock'>, volume: Volume): Promise<boolean> => {
			if (!client) return false;
			setBusyFor(volume.id, true);
			running.current += 1;
			if (action === 'eject') quiet.current.add(volume.id);
			try {
				if (action === 'mount') await client.mount(volume.id);
				else if (action === 'unmount') await client.unmount(volume.id);
				else await client.eject(volume.id);
				callbackRef.current.announce(tf(DONE[action], { name: volume.label }));
				return true;
			} catch (error) {
				quiet.current.delete(volume.id);
				fail(action, volume, error);
				return false;
			} finally {
				running.current -= 1;
				setBusyFor(volume.id, false);
			}
		},
		[client, fail, setBusyFor],
	);

	const unlock = useCallback(
		async (volume: Volume, passphrase: string): Promise<UnlockResult> => {
			if (!client) return 'failed';
			setBusyFor(volume.id, true);
			running.current += 1;
			try {
				const opened = await client.unlock(volume.id, passphrase);
				callbackRef.current.announce(tf('devices.announce.unlocked', { name: volume.label }));
				// The unlocked volume appears under a new id; mounting it is what the person wants next.
				try {
					await client.mount(opened);
				} catch (error) {
					fail('mount', volume, error);
				}
				return 'unlocked';
			} catch (error) {
				if (isVolumesError(error) && error.kind === 'wrongPassphrase') return 'wrong';
				fail('unlock', volume, error);
				return 'failed';
			} finally {
				running.current -= 1;
				setBusyFor(volume.id, false);
			}
		},
		[client, fail, setBusyFor],
	);

	const available = client !== null && status !== null && hasFeature(status, 'list');
	return useMemo(
		() => ({ status, available, volumes, busy, run, unlock }),
		[status, available, volumes, busy, run, unlock],
	);
}
