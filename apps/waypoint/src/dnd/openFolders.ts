// What a file drop on the + button or a group chip does: opens the dropped folders as tabs, or a split pair
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import { isSelected, type Selection } from '../browse/selection';
import { PAGE_SIZE, type ListingModel } from '../browse/listingModel';
import { t, tn } from '../i18n/messages';
import type { TabsApi } from '../services/tabsApi';
import type { VfsClient } from '../services/vfsClient';
import type { OpenFoldersRequest } from './fileDrag';
import { isLocationsSource } from './fileDragModel';

/** No more tabs than this open from one drop. */
export const MAX_DROP_TABS = 8;

export interface OpenFoldersDeps {
	api: TabsApi;
	vfs: VfsClient;
	snapshot(): SessionSnapshot | null;
	announce(text: string): void;
}

/** The selected entries of a listing, found a page at a time from the top, up to `limit`. */
export async function selectedEntries(
	model: ListingModel,
	selection: Selection,
	limit: number,
): Promise<Entry[]> {
	const found: Entry[] = [];
	for (let position = 0; position < model.count && found.length < limit;) {
		const end = Math.min(model.count, (Math.floor(position / PAGE_SIZE) + 1) * PAGE_SIZE);
		for (const entry of await model.readRange(position, end)) {
			if (isSelected(selection, entry.id)) found.push(entry);
			if (found.length >= limit) break;
		}
		position = end;
	}
	return found;
}

function isFolder(entry: Entry): boolean {
	return (
		entry.kind === 'directory' || (entry.kind === 'symlink' && entry.linkTarget === 'directory')
	);
}

/**
 * The folders a drop on + or a chip opens (SPEC §5.2): each dropped folder, or, when none of what
 * was dropped is a folder, the folder the files are in (a file opens its parent).
 */
export async function foldersToOpen(
	request: OpenFoldersRequest,
	vfs: VfsClient,
): Promise<Location[]> {
	const { source } = request;
	if (isLocationsSource(source)) {
		// Files from outside a listing; the Shelf's items are not dropped on + or a chip.
		return source.external
			? externalFoldersToOpen(source.external.locations, source.folder, vfs)
			: [];
	}
	if (!source.folder) return [];
	const handle = source.handle;
	const home = source.folder;
	const entries = await selectedEntries(
		source.session.model,
		source.session.store.getState().selection,
		MAX_DROP_TABS * 4,
	);
	const folders = entries.filter(isFolder).slice(0, MAX_DROP_TABS);
	if (folders.length === 0) return [home];
	return Promise.all(folders.map((entry) => vfs.entryLocation(handle, entry.id)));
}

/**
 * The same for files dragged in from another application: the dropped locations that are folders
 * (Rust says which), or else the folder the first file is in. A location that cannot be seen is
 * passed over.
 */
async function externalFoldersToOpen(
	files: readonly Location[],
	folder: Location | null,
	vfs: VfsClient,
): Promise<Location[]> {
	const checked = await Promise.all(
		files.slice(0, MAX_DROP_TABS * 4).map(async (file) => {
			try {
				return (await vfs.checkFolder(file)).isFolder ? file : null;
			} catch {
				return null;
			}
		}),
	);
	const folders = checked.filter((file): file is Location => file !== null).slice(0, MAX_DROP_TABS);
	if (folders.length > 0) return folders;
	if (folder) return [folder];
	const first = files[0];
	const parent = first
		? await vfs.describeLocation(first).then(
				(info) => info.parent,
				() => null,
			)
		: null;
	return parent ? [parent] : [];
}

/**
 * Opens the folders: new tabs after the active one (the first comes to the front, the rest stay
 * behind it), in the chip's group for a chip, or - with Alt on + - a split pair of the active
 * tab and a new tab at the first folder.
 */
export async function openFolders(deps: OpenFoldersDeps, request: OpenFoldersRequest) {
	const { api } = deps;
	const locations = await foldersToOpen(request, deps.vfs);
	if (locations.length === 0) {
		deps.announce(t('dnd.open.nothing'));
		return;
	}
	let after: number | undefined = deps.snapshot()?.active ?? undefined;
	const opened: number[] = [];
	const wanted = request.split && request.target === 'plus' ? locations.slice(0, 1) : locations;
	for (const [index, location] of wanted.entries()) {
		const id = await api.openTab(location, {
			...(after === undefined ? {} : { after }),
			activate: index === 0 && !request.split,
		});
		opened.push(id);
		after = id;
		if (request.target === 'chip' && request.group !== null) {
			await api.addToGroup(id, request.group);
		}
	}
	if (request.split && request.target === 'plus') {
		const active = deps.snapshot()?.active;
		if (active !== null && active !== undefined && opened[0] !== undefined) {
			await api.joinPair([active, opened[0]], 'sideBySide');
		}
	}
	deps.announce(tn('dnd.open.tabs', opened.length));
}
