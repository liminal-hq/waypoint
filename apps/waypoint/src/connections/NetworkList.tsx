// The sidebar's Network section: saved connections and recent servers, each with its state in words, and Connect to Server…
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ContextMenu } from '@liminal-hq/waypoint-chrome/ContextMenu';
import type { MenuItem, MenuPosition } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import { Dialog } from '@liminal-hq/waypoint-chrome/Dialog/Dialog';
import { DialogActions, DialogButton } from '@liminal-hq/waypoint-chrome/Dialog/DialogActions';
import type { ConnectionEntry } from '@liminal-hq/waypoint-protocol/generated/ConnectionEntry';
import type { ConnectionState } from '@liminal-hq/waypoint-protocol/generated/ConnectionState';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { RecentServer } from '@liminal-hq/waypoint-protocol/generated/RecentServer';
import {
	createElement,
	useEffect,
	useRef,
	useState,
	type KeyboardEvent,
	type MouseEvent,
} from 'react';
import { t, tf } from '../i18n/messages';
import { ITEM_ATTRIBUTE, moveFocusInList } from '../sidebar/itemList';
import sidebar from '../sidebar/Sidebar.module.css';
import { connectionErrorText, keyringText } from './connectModel';
import { useConnections, useConnectionsView } from './ConnectionsContext';
import { stateOf } from './connectionsModel';
import { DisconnectIcon, ServerIcon } from './ConnectionIcons';
import { openConnectDialog } from './connectStore';
import { stateTone, stateWords } from './remoteModel';
import styles from './NetworkList.module.css';

interface NetworkListProps {
	/** The login of the location the active tab shows, to mark the server it is on. */
	currentConnection: string | undefined;
	open(location: Location): void;
	openInNewTab(location: Location): void;
	/** Says something politely to a screen reader. */
	announce(message: string): void;
	/** Reports something that could not be done, for the status bar. */
	notice(message: string): void;
}

type Row =
	| { kind: 'saved'; entry: ConnectionEntry; index: number }
	| { kind: 'recent'; server: RecentServer };

interface MenuRequest {
	row: Row;
	position: MenuPosition;
	keyboard: boolean;
	returnFocusTo: HTMLElement | null;
}

const labelOf = (row: Row): string =>
	row.kind === 'saved' ? row.entry.label : row.server.location.display;
const keyOf = (row: Row): string => (row.kind === 'saved' ? row.entry.key : row.server.key);
const locationOf = (row: Row): Location =>
	row.kind === 'saved' ? row.entry.location : row.server.location;

/** The state's line under the name: the words, and for a failure what failed. */
function detailOf(state: ConnectionState): string {
	if (state.kind !== 'failed') return stateWords(state);
	return `${stateWords(state)} · ${connectionErrorText(state.error)}`;
}

/**
 * Saved connections first, in their saved order, then Recent servers. Each row's button opens the
 * server (connecting when it is shown, D146); its state is written under the name as well as shown
 * by the dot, and each change of state is announced politely. A connected server has a Disconnect
 * button; the rest is in the row's menu (right-click, the Menu key or Shift+F10): Open, Open in New
 * Tab, Connect or Disconnect, Edit…, Move Up and Move Down, and Delete… (asking first, starting on
 * Cancel) for a saved connection, and Save… and Forget for a recent server. Up, Down, Home and End
 * move between rows like the other lists. Connect to Server… ends the section.
 */
export function NetworkList({
	currentConnection,
	open,
	openInNewTab,
	announce,
	notice,
}: NetworkListProps) {
	const connections = useConnections();
	const saved = useConnectionsView((view) => view.connections);
	const recent = useConnectionsView((view) => view.recent);
	const view = useConnectionsView((view) => view);
	const [menu, setMenu] = useState<MenuRequest | null>(null);
	const [deleting, setDeleting] = useState<ConnectionEntry | null>(null);
	const [forgetLogin, setForgetLogin] = useState(true);
	useStateAnnouncements(saved, recent, view.states, announce);

	if (!connections) return null;
	const client = connections.client;
	const rows: Row[] = [
		...saved.map((entry, index) => ({ kind: 'saved' as const, entry, index })),
		...recent.map((server) => ({ kind: 'recent' as const, server })),
	];

	const disconnect = (row: Row) =>
		void client.disconnect(locationOf(row)).then(
			() => announce(tf('network.announce.disconnected', { name: labelOf(row) })),
			(error: unknown) => notice(connectionErrorText(error)),
		);
	const connect = (row: Row) =>
		void client.connect(locationOf(row)).catch(() => {
			// The state says why; a question is answered from the folder's own state when it opens.
		});

	const menuItems = (row: Row): MenuItem[] => {
		const state = stateOf(view, keyOf(row));
		const live = state.kind === 'connected' || state.kind === 'connecting';
		const items: MenuItem[] = [
			{ type: 'action', id: 'open', label: t('network.menu.open') },
			{ type: 'action', id: 'openInNewTab', label: t('network.menu.openInNewTab') },
			{ type: 'separator', id: 'sep-connect' },
			live
				? { type: 'action', id: 'disconnect', label: t('network.menu.disconnect') }
				: { type: 'action', id: 'connect', label: t('network.menu.connect') },
			{ type: 'separator', id: 'sep-edit' },
		];
		if (row.kind === 'saved') {
			items.push(
				{ type: 'action', id: 'edit', label: t('network.menu.edit') },
				{
					type: 'action',
					id: 'moveUp',
					label: t('network.menu.moveUp'),
					disabled: row.index === 0,
				},
				{
					type: 'action',
					id: 'moveDown',
					label: t('network.menu.moveDown'),
					disabled: row.index === saved.length - 1,
				},
				{ type: 'separator', id: 'sep-delete' },
				{ type: 'action', id: 'delete', label: t('network.menu.delete') },
			);
		} else {
			items.push(
				{ type: 'action', id: 'save', label: t('network.menu.save') },
				{ type: 'action', id: 'forget', label: t('network.menu.forget') },
			);
		}
		return items;
	};

	const run = (id: string, row: Row) => {
		switch (id) {
			case 'open':
				return open(locationOf(row));
			case 'openInNewTab':
				return openInNewTab(locationOf(row));
			case 'connect':
				return connect(row);
			case 'disconnect':
				return disconnect(row);
			case 'edit':
				if (row.kind === 'saved') {
					const id = row.entry.connection.id;
					setTimeout(() => openConnectDialog({ mode: 'edit', id }), 0);
				}
				return;
			case 'moveUp':
			case 'moveDown':
				if (row.kind !== 'saved') return;
				void client.move(row.entry.connection.id, row.index + (id === 'moveUp' ? -1 : 1)).then(
					() =>
						announce(
							tf('network.announce.moved', {
								name: row.entry.label,
								position: row.index + (id === 'moveUp' ? 0 : 2),
							}),
						),
					(error: unknown) => notice(connectionErrorText(error)),
				);
				return;
			case 'delete':
				if (row.kind === 'saved') {
					const entry = row.entry;
					setForgetLogin(true);
					// After the menu has closed and handed focus back, so the question takes it.
					setTimeout(() => setDeleting(entry), 0);
				}
				return;
			case 'save':
				setTimeout(() => openConnectDialog({ mode: 'new', address: locationOf(row).display }), 0);
				return;
			case 'forget':
				void client
					.forgetRecent(keyOf(row))
					.then(() => announce(tf('network.announce.forgotten', { name: labelOf(row) })));
				return;
		}
	};

	const remove = async () => {
		const entry = deleting;
		setDeleting(null);
		if (!entry) return;
		try {
			const why = await client.remove(entry.connection.id, forgetLogin);
			announce(
				why === null
					? tf('connect.result.removed', { name: entry.label })
					: tf('connect.result.removedKeyring', { name: entry.label, reason: keyringText(why) }),
			);
		} catch (error) {
			notice(connectionErrorText(error));
		}
	};

	const openMenu = (row: Row, event: MouseEvent<HTMLElement> | KeyboardEvent<HTMLElement>) => {
		const target = event.currentTarget;
		if ('clientX' in event) {
			event.preventDefault();
			setMenu({
				row,
				position: { x: event.clientX, y: event.clientY },
				keyboard: false,
				returnFocusTo: target,
			});
			return;
		}
		const rect = target.getBoundingClientRect();
		setMenu({
			row,
			position: { x: rect.left + 8, y: rect.bottom },
			keyboard: true,
			returnFocusTo: target,
		});
	};

	const onRowKey = (row: Row) => (event: KeyboardEvent<HTMLButtonElement>) => {
		if (event.key === 'ContextMenu' || (event.key === 'F10' && event.shiftKey)) {
			event.preventDefault();
			openMenu(row, event);
		}
	};

	const renderRow = (row: Row) => {
		const key = keyOf(row);
		const state = stateOf(view, key);
		const tone = stateTone(state);
		const label = labelOf(row);
		const location = locationOf(row);
		const live = state.kind === 'connected' || state.kind === 'connecting';
		const current = currentConnection === key;
		return (
			<li
				key={row.kind === 'saved' ? row.entry.connection.id : `recent-${row.server.key}`}
				className={styles.row}
			>
				<button
					type="button"
					{...{ [ITEM_ATTRIBUTE]: '' }}
					className={`${sidebar.item} ${styles.item}`}
					aria-current={current ? 'location' : undefined}
					title={location.display}
					onClick={() => open(location)}
					onAuxClick={(event) => {
						if (event.button === 1) openInNewTab(location);
					}}
					onContextMenu={(event) => openMenu(row, event)}
					onKeyDown={onRowKey(row)}
				>
					<span className={styles.iconWrap}>
						<ServerIcon className={sidebar.itemIcon} />
						<span className={styles.dot} data-tone={tone} aria-hidden="true" />
					</span>
					<span className={styles.text}>
						<span className={sidebar.label}>{label}</span>
						<span className={styles.detail}>{detailOf(state)}</span>
					</span>
				</button>
				{live && (
					<button
						type="button"
						className={styles.action}
						aria-label={tf('network.disconnect', { name: label })}
						title={tf('network.disconnect', { name: label })}
						onClick={() => disconnect(row)}
					>
						<DisconnectIcon />
					</button>
				)}
			</li>
		);
	};

	return (
		<>
			{saved.length === 0 && recent.length === 0 ? (
				<p className={sidebar.empty}>{t('network.empty')}</p>
			) : (
				<ul className={sidebar.list} aria-label={t('network.list')} onKeyDown={moveFocusInList}>
					{rows.filter((row) => row.kind === 'saved').map(renderRow)}
					{recent.length > 0 && (
						<li className={styles.subheading} aria-hidden="true">
							{t('network.recent')}
						</li>
					)}
					{rows.filter((row) => row.kind === 'recent').map(renderRow)}
				</ul>
			)}
			<button
				type="button"
				className={`${sidebar.item} ${styles.connect}`}
				onClick={() => openConnectDialog()}
			>
				{createElement(ServerIcon, { className: sidebar.itemIcon })}
				<span className={sidebar.label}>{t('cmd.connectToServer')}</span>
			</button>
			{menu && (
				<ContextMenu
					ariaLabel={tf('network.menu.label', { name: labelOf(menu.row) })}
					items={menuItems(menu.row)}
					position={menu.position}
					openedWithKeyboard={menu.keyboard}
					returnFocusTo={menu.returnFocusTo}
					onSelect={(item) => run(item.id, menu.row)}
					onClose={() => setMenu(null)}
				/>
			)}
			<Dialog
				open={deleting !== null}
				size="small"
				title={tf('connect.forget.title', { name: deleting?.label ?? '' })}
				description={t('connect.forget.message')}
				initialFocus="[data-cancel]"
				onClose={() => setDeleting(null)}
				footer={
					<DialogActions>
						<DialogButton variant="secondary" data-cancel="" onClick={() => setDeleting(null)}>
							{t('connect.cancel')}
						</DialogButton>
						<DialogButton variant="danger" onClick={() => void remove()}>
							{t('connect.forget.confirm')}
						</DialogButton>
					</DialogActions>
				}
			>
				<label className={styles.check}>
					<input
						type="checkbox"
						checked={forgetLogin}
						onChange={(event) => setForgetLogin(event.target.checked)}
					/>
					{t('connect.forget.login')}
				</label>
			</Dialog>
		</>
	);
}

/** Announces each change of state of a saved or recent server, politely, once per change. */
function useStateAnnouncements(
	saved: readonly ConnectionEntry[],
	recent: readonly RecentServer[],
	states: ReadonlyMap<string, ConnectionState>,
	announce: (message: string) => void,
) {
	const before = useRef<ReadonlyMap<string, ConnectionState> | null>(null);
	useEffect(() => {
		const previous = before.current;
		before.current = states;
		if (previous === null || previous === states) return;
		const names = new Map<string, string>();
		for (const server of recent) names.set(server.key, server.location.display);
		for (const entry of saved) names.set(entry.key, entry.label);
		for (const [key, state] of states) {
			const was = previous.get(key);
			if (was && stateTone(was) === stateTone(state)) continue;
			if (!was && state.kind === 'idle') continue;
			const name = names.get(key);
			if (name) announce(tf('network.announce.state', { name, state: stateWords(state) }));
		}
	}, [saved, recent, states, announce]);
}
