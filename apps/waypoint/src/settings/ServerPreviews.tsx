// The Previews & thumbnails page's servers: whether each saved connection shows previews of its files
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ConnectionEntry } from '@liminal-hq/waypoint-protocol/generated/ConnectionEntry';
import type { RemoteThumbnails } from '@liminal-hq/waypoint-protocol/generated/RemoteThumbnails';
import { SelectRow } from '@liminal-hq/waypoint-chrome/SettingsShell/SelectRow';
import { SettingsGroup } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsGroup';
import { createContext, useContext, useEffect, useState, type ReactNode } from 'react';
import type { ConnectionsClient } from '../connections/connectionsClient';
import { t, tf } from '../i18n/messages';
import { announce } from '../tabs/announcer';

/** Reads the saved connections and changes one's previews choice; the connections plugin's in the app. */
export interface ServerPreviewsClient {
	list(): Promise<ConnectionEntry[]>;
	setThumbnails(entry: ConnectionEntry, choice: RemoteThumbnails): Promise<ConnectionEntry>;
}

/** The client over the connections commands: a change is the connection saved again with only that option changed. */
export function createServerPreviewsClient(
	client: Pick<ConnectionsClient, 'list' | 'update'>,
): ServerPreviewsClient {
	return {
		list: async () => [...(await client.list()).connections.connections],
		setThumbnails: (entry, choice) => {
			const { id, ...draft } = entry.connection;
			return client.update(id, { ...draft, options: { ...draft.options, thumbnails: choice } });
		},
	};
}

const ServerPreviewsContext = createContext<ServerPreviewsClient | null>(null);

export function ServerPreviewsProvider({
	client,
	children,
}: {
	client: ServerPreviewsClient | null;
	children: ReactNode;
}) {
	return <ServerPreviewsContext.Provider value={client}>{children}</ServerPreviewsContext.Provider>;
}

const CHOICES: RemoteThumbnails[] = ['off', 'smallFiles', 'always'];

/**
 * One row for each saved connection, with its previews choice (D166): off unless turned on, small
 * files only, or always (with a large photo's own small preview). A server not saved shows none.
 * Nothing is listed where the window cannot reach the connections, and a list of none says how to
 * add one. A change is saved at once and the row shows what Rust kept.
 */
export function ServerPreviewsGroup() {
	const client = useContext(ServerPreviewsContext);
	const [entries, setEntries] = useState<ConnectionEntry[] | null>(null);
	const [errors, setErrors] = useState<Record<string, string>>({});
	useEffect(() => {
		if (!client) return;
		let live = true;
		client.list().then(
			(list) => live && setEntries(list),
			(error: unknown) => console.warn('could not read the saved connections', error),
		);
		return () => {
			live = false;
		};
	}, [client]);
	if (!client || entries === null) return null;

	const change = async (entry: ConnectionEntry, choice: RemoteThumbnails) => {
		const id = entry.connection.id;
		setErrors(({ [id]: _, ...rest }) => rest);
		try {
			const saved = await client.setThumbnails(entry, choice);
			setEntries((now) => now?.map((e) => (e.connection.id === id ? saved : e)) ?? null);
			announce(
				tf('settings.previews.servers.changed', {
					server: saved.label,
					choice: t(`connect.thumbnails.${saved.connection.options.thumbnails}`),
				}),
			);
		} catch (error) {
			console.warn('could not change a connection', error);
			setErrors((now) => ({ ...now, [id]: t('settings.previews.servers.failed') }));
		}
	};

	return (
		<SettingsGroup title={t('settings.group.serverPreviews')}>
			{entries.length === 0 ? (
				<p>{t('settings.previews.servers.none')}</p>
			) : (
				entries.map((entry) => (
					<SelectRow
						key={entry.connection.id}
						label={entry.label}
						description={t('connect.field.thumbnailsHint')}
						value={entry.connection.options.thumbnails}
						options={CHOICES.map((choice) => ({
							value: choice,
							label: t(`connect.thumbnails.${choice}`),
						}))}
						error={errors[entry.connection.id]}
						onChange={(choice) => void change(entry, choice)}
					/>
				))
			)}
		</SettingsGroup>
	);
}
