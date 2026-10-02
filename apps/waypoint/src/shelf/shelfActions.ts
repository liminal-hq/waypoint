// What the Shelf does: add and remove, open and reveal, copy, check which files are still there, and tidy up after a move
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { ShelfItem } from '@liminal-hq/waypoint-protocol/generated/ShelfItem';
import type { ShelfItemId } from '@liminal-hq/waypoint-protocol/generated/ShelfItemId';
import { selectedEntries } from '../dnd/openFolders';
import type { ListingSession } from '../browse/useListingSession';
import { t, tf, tn } from '../i18n/messages';
import type { ClipboardService } from '../ops/clipboardService';
import { SHELF_LIMIT } from '../services/fakeTabsStore';
import { shelfFullLimit, type TabsApi } from '../services/tabsApi';
import { isVfsError, type VfsClient } from '../services/vfsClient';
import type { ItemState } from './shelfModel';
import type { ShelfStore } from './shelfStore';

/** How many files are asked about at once, so a long Shelf does not flood the file system. */
const CHECK_BATCH = 6;

/** What the actions need from the window. */
export interface ShelfDeps {
	store: ShelfStore;
	api: Pick<
		TabsApi,
		| 'addToShelf'
		| 'removeFromShelf'
		| 'clearShelf'
		| 'openTab'
		| 'navigate'
		| 'setTabHints'
		| 'activateTab'
	>;
	vfs: Pick<VfsClient, 'checkFolder' | 'entryLocation'>;
	/** A message in the status bar's toast, which is read out too. */
	say(text: string): void;
	/** The live region. */
	announce(text: string): void;
	activeTab(): number | null;
	/** The shared clipboard, where this window has one. */
	clipboard(): ClipboardService | null;
	writeText(text: string): Promise<void>;
}

export interface ShelfActions {
	/** Puts locations on the Shelf and says what happened (a full Shelf says so and adds nothing). Resolves to how many were new. */
	add(locations: readonly Location[]): Promise<number>;
	/** Puts the selection of a listing on the Shelf as references. */
	addSelection(session: ListingSession): Promise<number>;
	remove(ids: readonly ShelfItemId[]): Promise<void>;
	clear(): Promise<void>;
	/** A folder opens in the active tab; a file shows in its folder (`reveal`), because files open from a listing. */
	open(item: ShelfItem): Promise<void>;
	/** Opens the item's folder in a new tab with the item focused. */
	reveal(item: ShelfItem): Promise<void>;
	copyPath(items: readonly ShelfItem[]): Promise<void>;
	/** Puts the items on the shared clipboard as a copy, so Paste puts them in a folder. */
	copyFiles(items: readonly ShelfItem[]): Promise<void>;
	/** Finds out which items are still there, a few at a time; each result lands in the store as it arrives. */
	check(items: readonly ShelfItem[], options?: { force?: boolean }): Promise<void>;
	/**
	 * After a move out of the folders the items were in: takes off the ones whose files are no
	 * longer where they were (the Shelf points at a place, and the file left it), and keeps the
	 * rest (a conflict that skipped a file leaves it there).
	 */
	afterMove(locations: readonly Location[]): Promise<void>;
}

export function createShelfActions(deps: ShelfDeps): ShelfActions {
	const { store, api, vfs } = deps;
	let running: Promise<void> | null = null;
	/** What was asked for while a look ran, by file; a forced request keeps its force. */
	const queued = new Map<string, { item: ShelfItem; force: boolean }>();

	const fail = (error: unknown) => {
		const limit = shelfFullLimit(error);
		if (limit !== null) {
			deps.say(tf('shelf.full', { limit }));
			return;
		}
		const reason =
			error instanceof Error
				? error.message
				: typeof error === 'object' && error !== null && 'message' in error
					? String((error as { message: unknown }).message)
					: String(error);
		console.warn('the Shelf could not be changed', error);
		deps.say(tf('shelf.failed', { reason }));
	};

	const stateOf = async (location: Location): Promise<ItemState | null> => {
		try {
			const found = await vfs.checkFolder(location);
			return found.isFolder ? 'folder' : 'file';
		} catch (error) {
			// Only a file that is not there is missing; a permission or an I/O error says nothing about it.
			return isVfsError(error) && error.kind === 'notFound' ? 'missing' : null;
		}
	};

	const checkNow = async (items: readonly ShelfItem[]): Promise<void> => {
		for (let at = 0; at < items.length; at += CHECK_BATCH) {
			const batch = items.slice(at, at + CHECK_BATCH);
			const found = await Promise.all(batch.map((item) => stateOf(item.location)));
			const entries = new Map<string, ItemState>();
			batch.forEach((item, index) => {
				const state = found[index];
				if (state) entries.set(item.location.uri, state);
			});
			store.getState().setStatus(entries);
		}
	};

	const actions: ShelfActions = {
		async add(locations) {
			if (locations.length === 0) return 0;
			const have = new Set(store.getState().items.map((item) => item.location.uri));
			const fresh = new Set(locations.map((l) => l.uri).filter((uri) => !have.has(uri))).size;
			try {
				await api.addToShelf([...locations]);
			} catch (error) {
				fail(error);
				return 0;
			}
			if (fresh === 0) deps.say(t('shelf.added.already'));
			else deps.say(tn('shelf.added', fresh));
			return fresh;
		},

		async addSelection(session) {
			const { model, store: listing } = session;
			const entries = await selectedEntries(
				model,
				listing.getState().selection,
				SHELF_LIMIT + 1,
			).catch((error: unknown) => {
				fail(error);
				return [];
			});
			const locations: Location[] = [];
			try {
				for (const entry of entries)
					locations.push(await vfs.entryLocation(model.handle, entry.id));
			} catch (error) {
				fail(error);
				return 0;
			}
			return actions.add(locations);
		},

		async remove(ids) {
			if (ids.length === 0) return;
			const present = new Set(store.getState().items.map((item) => item.id));
			const count = ids.filter((id) => present.has(id)).length;
			try {
				await api.removeFromShelf([...ids]);
			} catch (error) {
				fail(error);
				return;
			}
			if (count > 0) deps.announce(tn('shelf.removed', count));
		},

		async clear() {
			try {
				await api.clearShelf();
			} catch (error) {
				fail(error);
				return;
			}
			deps.announce(t('shelf.cleared'));
		},

		async open(item) {
			const known = store.getState().status.get(item.location.uri);
			const tab = deps.activeTab();
			if (known === 'folder' && tab !== null) {
				await api.navigate(tab, item.location).catch(fail);
				return;
			}
			return actions.reveal(item);
		},

		async reveal(item) {
			try {
				const tab = await api.openTab(item.origin, { activate: true });
				await api.setTabHints(tab, { scrollTop: 0, focused: item.name });
			} catch (error) {
				console.warn('could not reveal the Shelf item', error);
				deps.say(tf('shelf.reveal.failed', { name: item.name }));
			}
		},

		async copyPath(items) {
			if (items.length === 0) return;
			try {
				await deps.writeText(items.map((item) => item.location.display).join('\n'));
			} catch (error) {
				fail(error);
			}
		},

		async copyFiles(items) {
			const clipboard = deps.clipboard();
			if (!clipboard || items.length === 0) return;
			try {
				await clipboard.setFromLocations(
					items.map((item) => item.location),
					'copy',
				);
			} catch (error) {
				console.warn('could not copy from the Shelf', error);
				deps.say(
					tf('shelf.copyFailed', {
						reason: error instanceof Error ? error.message : String(error),
					}),
				);
				return;
			}
			deps.say(tn('shelf.copied', items.length));
		},

		async check(items, options = {}) {
			const status = store.getState().status;
			const wanted = options.force ? items : items.filter((item) => !status.has(item.location.uri));
			if (wanted.length === 0) return;
			for (const item of wanted) {
				const known = queued.get(item.location.uri);
				queued.set(item.location.uri, {
					item,
					force: (known?.force ?? false) || options.force === true,
				});
			}
			// One look at a time, but nothing asked for is dropped: what arrives while one runs is
			// looked at after it (and only if it still has no answer, unless forced), so an item added or
			// rechecked meanwhile is never left "unknown".
			running ??= Promise.resolve().then(async () => {
				try {
					while (queued.size > 0) {
						const requests = [...queued.values()];
						queued.clear();
						const status = store.getState().status;
						const todo = requests
							.filter((request) => request.force || !status.has(request.item.location.uri))
							.map((request) => request.item);
						if (todo.length > 0) await checkNow(todo);
					}
				} finally {
					// Set here, with no await since the queue was found empty, so no request slips in unseen.
					running = null;
				}
			});
			return running;
		},

		async afterMove(locations) {
			if (locations.length === 0) return;
			const uris = new Set(locations.map((l) => l.uri));
			const moved = store.getState().items.filter((item) => uris.has(item.location.uri));
			const gone: ShelfItemId[] = [];
			for (const item of moved) {
				if ((await stateOf(item.location)) === 'missing') gone.push(item.id);
			}
			if (gone.length > 0) await actions.remove(gone);
		},
	};
	return actions;
}
