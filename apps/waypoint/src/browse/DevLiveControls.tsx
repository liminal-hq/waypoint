// Development-only buttons that change a fake folder under an open listing, to watch live updates
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { useEffect, useRef, useState } from 'react';
import { t } from '../i18n/messages';
import { makeEntry, type FakeVfsClient } from '../services/fakeVfsClient';
import styles from './DevLiveControls.module.css';

interface DevLiveControlsProps {
	client: FakeVfsClient;
	location: Location;
}

/** The entries a folder holds, read back through a throwaway listing (the fake keeps them private). */
async function currentEntries(client: FakeVfsClient, location: Location): Promise<Entry[]> {
	const snapshot = await client.openListing(location, {
		sort: { key: 'name', descending: false, directoriesFirst: true },
		filter: { showHidden: true },
	});
	const entries = await client.getRange(snapshot.handle, 0, snapshot.count);
	await client.closeListing(snapshot.handle);
	return entries;
}

/**
 * Each button mutates the fake folder the way a file watcher would, so the open list can be seen
 * keeping its scroll position and selection. Mounted only when `import.meta.env.DEV` is set.
 */
export function DevLiveControls({ client, location }: DevLiveControlsProps) {
	const nextId = useRef(1_000_000);
	const [auto, setAuto] = useState(false);

	const add = (atTop: boolean) => {
		const fresh = Array.from({ length: 5 }, () => {
			const id = nextId.current++;
			return makeEntry(id, `${atTop ? '0000-new' : 'zzzz-new'}-${id}.txt`);
		});
		client.addEntries(location, fresh);
	};

	const pick = async (): Promise<Entry[]> => {
		const entries = await currentEntries(client, location);
		const start = Math.floor(Math.random() * Math.max(1, entries.length - 5));
		return entries.slice(start, start + 5);
	};

	const remove = async () =>
		client.removeEntries(
			location,
			(await pick()).map((e) => e.id),
		);
	const touch = async () =>
		client.updateEntries(
			location,
			new Map(
				(await pick()).map((e) => [
					e.id,
					{ modifiedMs: Date.now(), size: e.size === null ? null : e.size + 1 },
				]),
			),
		);

	useEffect(() => {
		if (!auto) return;
		const timer = setInterval(() => {
			const roll = Math.random();
			if (roll < 0.34) add(Math.random() < 0.5);
			else if (roll < 0.67) void remove();
			else void touch();
		}, 700);
		return () => clearInterval(timer);
	}, [auto]);

	return (
		<div className={styles.bar} role="group" aria-label={t('dev.live.label')}>
			<button type="button" onClick={() => add(false)}>
				{t('dev.live.add')}
			</button>
			<button type="button" onClick={() => add(true)}>
				{t('dev.live.addTop')}
			</button>
			<button type="button" onClick={() => void remove()}>
				{t('dev.live.remove')}
			</button>
			<button type="button" onClick={() => void touch()}>
				{t('dev.live.touch')}
			</button>
			<label>
				<input type="checkbox" checked={auto} onChange={(e) => setAuto(e.target.checked)} />
				{t('dev.live.auto')}
			</label>
		</div>
	);
}
