// Reads the applications for an entry's menu and runs the Open With row that is chosen
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Handlers } from '@liminal-hq/plugin-mime-apps';
import type { SubmenuMenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { showNotice } from '../app/notices';
import type { ListingSession } from '../browse/useListingSession';
import { useOptionalVfsClient } from '../browse/VfsClientContext';
import { openWithAbilities, openWithOffered, useOpenWithService } from './OpenWithContext';
import { openWithAction, openWithMenu } from './openWithMenu';
import { openWithChooserStore, type OpenWithChooserStore } from './openWithChooserStore';
import { openWithUris } from './openWithUris';
import { startOpenWith } from './startOpenWith';

/** What the menu is for: the entry that was right-clicked, in its listing. */
export interface OpenWithTarget {
	session: ListingSession | null | undefined;
	entry: Entry;
	handle: ListingHandle;
}

export interface OpenWithMenu {
	/** The submenu to add to the entry menu, or `null` where Open With is not offered. */
	item: SubmenuMenuItem | null;
	/** Runs a chosen row; false when `id` is not one of the submenu's. */
	select(id: string): boolean;
}

type Read =
	| { state: 'loading' }
	| { state: 'hidden' }
	| { state: 'ready'; uris: string[]; handlers: Handlers };

const NO_APPS: Handlers = { mime: '', mixed: false, default: null, recommended: [], others: [] };

/**
 * The Open With submenu for the entry menu. The applications are read when the menu opens, off the
 * plugin's answer for the selection's type, so the submenu first holds a disabled row and
 * then the applications. Nothing is offered where the plugin cannot do it (`getStatus`), for a
 * selection of more than one type, or where the files are not on this computer.
 */
export function useOpenWithMenu(
	target: OpenWithTarget,
	chooser: OpenWithChooserStore = openWithChooserStore,
): OpenWithMenu {
	const service = useOpenWithService();
	const vfs = useOptionalVfsClient();
	const status = service?.status ?? null;
	const client = service?.client ?? null;
	const offered = client !== null && vfs !== null && openWithOffered(status);
	const [read, setRead] = useState<Read>({ state: 'loading' });
	const { session, entry, handle } = target;

	useEffect(() => {
		if (!client || !vfs || !offered) {
			setRead({ state: 'hidden' });
			return;
		}
		let live = true;
		setRead({ state: 'loading' });
		(async () => {
			const uris = await openWithUris(vfs, session ?? null, entry, handle);
			if (!uris) return null;
			const handlers = await client.handlers(uris).catch((error: unknown) => {
				// With the system's chooser to fall back on, a failed list is not the end of Open With.
				console.warn('could not read the applications', error);
				return openWithAbilities(status).chooser ? NO_APPS : null;
			});
			return handlers ? { uris, handlers } : null;
		})().then(
			(found) => {
				if (live) setRead(found ? { state: 'ready', ...found } : { state: 'hidden' });
			},
			(error: unknown) => {
				console.warn('could not find what Open With acts on', error);
				if (live) setRead({ state: 'hidden' });
			},
		);
		return () => {
			live = false;
		};
	}, [client, offered, status, vfs, session, entry, handle]);

	const item = useMemo(
		() =>
			read.state === 'hidden' || !client
				? null
				: openWithMenu({
						status,
						handlers: read.state === 'ready' ? read.handlers : null,
						count: read.state === 'ready' ? read.uris.length : 1,
						iconUrl: (appId) => client.iconUrl(appId),
					}),
		[read, client, status],
	);

	const select = useCallback(
		(id: string) => {
			if (read.state !== 'ready' || !client) return false;
			const action = openWithAction(id, read.handlers);
			if (!action) return false;
			if (action.kind === 'other') {
				chooser.getState().open({ uris: read.uris, handlers: read.handlers, scope: 'others' });
			} else {
				void startOpenWith(client, read.uris, action, showNotice);
			}
			return true;
		},
		[read, client, chooser],
	);

	return { item, select };
}
