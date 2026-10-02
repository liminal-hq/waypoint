// What a Properties window is about, and whether it still exists: the entry is found in a listing of its folder and followed through renames
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { EntryId } from '@liminal-hq/waypoint-protocol/generated/EntryId';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { useEffect, useState } from 'react';
import { openListingModel, PAGE_SIZE, type ListingModel } from '../browse/listingModel';
import type { PropertiesWindowClient } from '../services/propertiesWindowClient';
import type { VfsClient } from '../services/vfsClient';
import { baseName } from './inspectorModel';

/** How long after a change in the folder the entry is looked for again; a burst of changes is one look. */
export const FOLLOW_DELAY_MS = 150;

export type WindowSubject =
	| { status: 'loading' }
	/** The entry, in the listing of its folder (`parent`). */
	| { status: 'entry'; handle: ListingHandle; entry: Entry; location: Location; parent: Location }
	/** A root has no folder to list it in, so there is no entry to read. */
	| { status: 'root'; location: Location }
	/** It was removed, moved away or renamed out of reach; `name` is what it was called last. */
	| { status: 'gone'; name: string; location: Location }
	| { status: 'failed' };

/** Waits until a listing has stopped scanning, so a name that is not there yet is not taken for one that is not there. */
function scanned(model: ListingModel): Promise<void> {
	if (model.phase !== 'scanning') return Promise.resolve();
	return new Promise((resolve) => {
		const off = model.subscribe(() => {
			if (model.phase === 'scanning') return;
			off();
			resolve();
		});
	});
}

/**
 * Finds the entry called `name`, a page at a time starting at the page of `hint` (where it was
 * last), so following a busy folder reads one page and not all of them.
 */
export async function findByName(
	model: ListingModel,
	name: string,
	hint = 0,
): Promise<{ entry: Entry; position: number } | null> {
	const pages = Math.ceil(model.count / PAGE_SIZE);
	if (pages === 0) return null;
	const first = Math.min(pages - 1, Math.floor(hint / PAGE_SIZE));
	for (let step = 0; step < pages; step++) {
		const page = (first + step) % pages;
		const from = page * PAGE_SIZE;
		const entries = await model.readRange(from, from + PAGE_SIZE);
		const at = entries.findIndex((entry) => entry.name === name);
		if (at >= 0) return { entry: entries[at]!, position: from + at };
	}
	return null;
}

/**
 * Reads the subject the window was opened for and follows it. The window lists the folder the
 * entry is in (hidden files included, since the subject may be one) and finds the entry by name;
 * from then on it is known by its id, which a rename keeps, so a rename gives the new name (and is
 * reported to Rust, so a request for the new name finds this window) and a removal gives
 * `gone`. Everything is dropped when the window goes away.
 */
export function usePropertiesWindowSubject(
	vfs: VfsClient,
	windows: PropertiesWindowClient,
): WindowSubject {
	const [subject, setSubject] = useState<WindowSubject>({ status: 'loading' });

	useEffect(() => {
		let live = true;
		let model: ListingModel | null = null;
		let off = () => {};
		let timer: ReturnType<typeof setTimeout> | undefined;

		const run = async () => {
			const location = await windows.subject();
			const info = await vfs.describeLocation(location);
			if (!live) return;
			if (!info.parent) {
				setSubject({ status: 'root', location });
				return;
			}
			const parent = info.parent;
			const opened = await openListingModel(vfs, parent, { filter: { showHidden: true } });
			if (!live) {
				opened.dispose();
				return;
			}
			model = opened;
			await scanned(opened);
			if (!live) return;
			let name = baseName(location.display);
			let known = await findByName(opened, name);
			if (!live) return;
			if (!known) {
				setSubject({ status: 'gone', name, location });
				return;
			}
			let id: EntryId = known.entry.id;
			let current = location;
			setSubject({
				status: 'entry',
				handle: opened.handle,
				entry: known.entry,
				location: current,
				parent,
			});

			const follow = async () => {
				if (!live) return;
				try {
					if (opened.phase === 'failed') throw new Error('the folder cannot be listed');
					const now = await vfs.entryLocation(opened.handle, id);
					if (!live) return;
					name = baseName(now.display);
					if (now.uri !== current.uri) {
						current = now;
						await windows.setSubject(now).catch((error: unknown) => {
							console.warn('could not record the new subject', error);
						});
					}
					const found = await findByName(opened, name, known?.position ?? 0);
					if (!live) return;
					if (!found) throw new Error('the entry is not listed');
					known = found;
					id = found.entry.id;
					setSubject({
						status: 'entry',
						handle: opened.handle,
						entry: found.entry,
						location: current,
						parent,
					});
				} catch {
					if (live) setSubject({ status: 'gone', name, location: current });
				}
			};

			const offPatch = opened.onPatch(() => {
				clearTimeout(timer);
				timer = setTimeout(() => void follow(), FOLLOW_DELAY_MS);
			});
			// A folder that can no longer be listed (it was removed) takes its entries with it.
			const offFailed = opened.subscribe(() => {
				if (opened.phase === 'failed') void follow();
			});
			off = () => {
				offPatch();
				offFailed();
			};
		};

		run().catch((error: unknown) => {
			console.warn('could not read the Properties subject', error);
			if (live) setSubject({ status: 'failed' });
		});

		return () => {
			live = false;
			clearTimeout(timer);
			off();
			model?.dispose();
		};
	}, [vfs, windows]);

	return subject;
}
