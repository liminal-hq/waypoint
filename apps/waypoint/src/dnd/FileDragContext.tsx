// Supplies the window's one file drag to the views that start it, and draws the ghost and the action picker
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import {
	createContext,
	useContext,
	useEffect,
	useMemo,
	useRef,
	useState,
	type ReactNode,
} from 'react';
import { frameMargin, outsideVisibleWindow } from '../app/frameMargin';
import { showNotice } from '../app/notices';
import type { ListingManager } from '../browse/listingManager';
import { useVfsClient } from '../browse/VfsClientContext';
import { useConnectionsView } from '../connections/ConnectionsContext';
import { serverLabel } from '../connections/connectionsModel';
import { t } from '../i18n/messages';
import { useFileCommands } from '../ops/FileCommandsContext';
import { useOps } from '../ops/OpsContext';
import {
	NO_NATIVE_DND,
	type NativeDndAvailability,
	type NativeDndClient,
} from '../services/nativeDndClient';
import { useSettings } from '../settings/SettingsContext';
import { useShelfActions } from '../shelf/ShelfContext';
import { useTrashActions } from '../trash/trashJobs';
import { announce } from '../tabs/announcer';
import { useSeparateSession, useTabDragSession } from '../tabs/TabDragContext';
import { useTabsApi, useTabsSnapshot } from '../tabs/TabsContext';
import { ActionPicker } from './ActionPicker';
import { DragStack } from './DragStack';
import {
	createFileDrag,
	type FileDrag,
	type FileDragPress,
	type LocationsDragPress,
	type PickerRequest,
} from './fileDrag';
import { isLocationsSource } from './fileDragModel';
import { connectNativeDnd } from './nativeDndHost';
import { openFolders } from './openFolders';
import './dropTargets.css';

/** What a file view uses: start a drag from a press, hold a right-click menu back, and swallow the click that ends a drag. */
export interface FileDragApi {
	press(press: FileDragPress): boolean;
	/** Starts a drag of references (the Shelf's rows) rather than of a listing's selection. */
	pressLocations(press: LocationsDragPress): boolean;
	deferMenu(open: () => void): boolean;
	consumeClick(): boolean;
}

const FileDragContext = createContext<FileDragApi | null>(null);

/** The file drag, or `null` outside a provider (a view on its own, with no window around it). */
export function useFileDragApi(): FileDragApi | null {
	return useContext(FileDragContext);
}

interface FileDragProviderProps {
	/** Holds the source pane's listing open while a drag springs it elsewhere. */
	manager: ListingManager;
	/**
	 * The native drag and drop plugin. Where it works, files dragged in from other applications and
	 * windows drop on the same targets, and a drag that leaves the window continues as a system
	 * drag; without it (or where it does not work) drags stay in the page.
	 */
	nativeDnd?: NativeDndClient;
	children: ReactNode;
}

/**
 * One file drag per window, over the shared drag session (`dragSession.ts`). It reads the
 * settings (`dnd.default_action_rule`, `dnd.spring_load_ms`) as the drag runs, so a change takes
 * effect on the next move. A file drag and a tab drag are different sessions and never run
 * together: a press waits while the tab drag has the pointer.
 */
export function FileDragProvider({ manager, nativeDnd, children }: FileDragProviderProps) {
	const api = useTabsApi();
	const snapshot = useTabsSnapshot();
	const vfs = useVfsClient();
	const ops = useOps();
	const trash = useTrashActions();
	const commands = useFileCommands();
	const shelf = useShelfActions();
	const tabDrag = useTabDragSession();
	const separate = useSeparateSession();
	const rule = useSettings((settings) => settings.dnd.defaultActionRule);
	const springMs = useSettings((settings) => settings.dnd.springLoadMs);
	const connections = useConnectionsView((view) => view);
	const [picker, setPicker] = useState<PickerRequest | null>(null);
	// What the plugin can do here, read once; the drag asks as it runs.
	const features = useRef<NativeDndAvailability>(NO_NATIVE_DND);

	// The drag is made once; what it reads changes under it.
	const latest = useRef({
		api,
		snapshot,
		vfs,
		ops,
		trash,
		commands,
		shelf,
		rule,
		springMs,
		manager,
		connections,
	});
	latest.current = {
		api,
		snapshot,
		vfs,
		ops,
		trash,
		commands,
		shelf,
		rule,
		springMs,
		manager,
		connections,
	};

	const [drag] = useState<FileDrag>(() => {
		const now = () => latest.current;
		const tabOf = (tab: number) => now().snapshot?.tabs.find((candidate) => candidate.id === tab);
		return createFileDrag({
			rule: () => now().rule,
			springMs: () => now().springMs,
			announce,
			say: (text) => void showNotice(text),
			windowLabel: () => now().ops?.windowLabel ?? null,
			plan: (request) => {
				const handle = now().ops?.handle;
				return handle ? handle.client.plan(request) : Promise.reject(new Error('no queue'));
			},
			entryLocation: (handle, entry) => now().vfs.entryLocation(handle, entry),
			pane: (tab) => {
				const found = tabOf(tab);
				if (!found) return null;
				const state = now().manager.stateFor(tab);
				return {
					location: found.location,
					readOnly: state?.status === 'ready' ? state.session.model.readOnly : false,
				};
			},
			tabLocation: (tab) => tabOf(tab)?.location ?? null,
			activeTab: () => now().snapshot?.active ?? null,
			navigate: (tab, location) => now().api.navigate(tab, location),
			back: (tab) => now().api.back(tab),
			activate: (tab) => now().api.activateTab(tab),
			retain: (tab) => now().manager.retain(tab),
			transfer: async (kind, session, destination) => {
				await now().commands?.transferTo(kind, session, destination);
			},
			moveToTrash: async (session) => {
				await now().commands?.moveToTrash(session);
			},
			transferLocations: async (kind, items, destination) => {
				const job = await now().commands?.transferLocations(kind, items, destination);
				// A move took the files out of where the Shelf points: the ones that left have no entry to keep.
				if (kind === 'move' && job?.state.state === 'done') await now().shelf?.afterMove(items);
			},
			addToShelf: async (source) => {
				if (isLocationsSource(source)) await now().shelf?.add(source.locations);
				else await now().shelf?.addSelection(source.session);
			},
			openFolders: (request) =>
				openFolders(
					{
						api: now().api,
						vfs: now().vfs,
						snapshot: () => now().snapshot,
						announce,
					},
					request,
				).catch((error: unknown) => {
					console.warn('could not open the dropped folders', error);
					void showNotice(t('dnd.open.failed'));
				}),
			openPicker: setPicker,
			trashAvailable: () => now().trash !== null,
			serverName: (login) => serverLabel(now().connections, login),
			...(nativeDnd
				? {
						outbound: {
							available: () => features.current.outbound,
							outside: (point) =>
								outsideVisibleWindow(
									point,
									{ width: window.innerWidth, height: window.innerHeight },
									frameMargin(),
								),
							resolve: (handle, spec) => {
								const queue = now().ops;
								return queue
									? queue.handle.client.resolveSelection(handle, spec)
									: Promise.reject(new Error('no queue'));
							},
							stage: (locations) => {
								const queue = now().ops;
								return queue
									? queue.handle.client.stageForDrag(locations)
									: Promise.reject(new Error('no queue'));
							},
							start: (request) => nativeDnd.startDrag(request),
						},
					}
				: {}),
		});
	});
	useEffect(() => () => drag.dispose(), [drag]);
	const windowLabel = ops?.windowLabel ?? null;
	const labelRef = useRef(windowLabel);
	labelRef.current = windowLabel;
	useEffect(() => {
		if (!nativeDnd) return;
		return connectNativeDnd({
			client: nativeDnd,
			drag,
			windowLabel: () => labelRef.current,
			onAvailability: (found) => {
				features.current = found;
				for (const [feature, reason] of Object.entries(found.reasons)) {
					console.info(`native drag and drop: ${feature} is unavailable: ${reason}`);
				}
			},
		});
	}, [nativeDnd, drag]);

	const value = useMemo<FileDragApi>(() => {
		// A tab drag, or a pane's grip, has the pointer: a file drag does not begin beside it.
		const busy = () =>
			[tabDrag.store.getState().phase, separate.store.getState().phase].some(
				(phase) => phase === 'pending' || phase === 'dragging',
			);
		return {
			press: (press) => (busy() ? false : drag.press(press)),
			pressLocations: (press) => (busy() ? false : drag.pressLocations(press)),
			deferMenu: (open) => drag.deferMenu(open),
			consumeClick: () => drag.session.consumeClick(),
		};
	}, [drag, tabDrag, separate]);

	return (
		<FileDragContext.Provider value={value}>
			{children}
			<DragStack session={drag.session} />
			{picker && <ActionPicker request={picker} onDone={() => setPicker(null)} />}
		</FileDragContext.Provider>
	);
}
