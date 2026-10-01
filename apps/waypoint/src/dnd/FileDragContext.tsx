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
import { showNotice } from '../app/notices';
import type { ListingManager } from '../browse/listingManager';
import { useVfsClient } from '../browse/VfsClientContext';
import { t } from '../i18n/messages';
import { useFileCommands } from '../ops/FileCommandsContext';
import { useOps } from '../ops/OpsContext';
import { useSettings } from '../settings/SettingsContext';
import { useTrashActions } from '../trash/trashJobs';
import { announce } from '../tabs/announcer';
import { useSeparateSession, useTabDragSession } from '../tabs/TabDragContext';
import { useTabsApi, useTabsSnapshot } from '../tabs/TabsContext';
import { ActionPicker } from './ActionPicker';
import { DragStack } from './DragStack';
import { createFileDrag, type FileDrag, type FileDragPress, type PickerRequest } from './fileDrag';
import { openFolders } from './openFolders';
import './dropTargets.css';

/** What a file view uses: start a drag from a press, hold a right-click menu back, and swallow the click that ends a drag. */
export interface FileDragApi {
	press(press: FileDragPress): boolean;
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
	children: ReactNode;
}

/**
 * One file drag per window, over the shared drag session (`dragSession.ts`). It reads the
 * settings (`dnd.default_action_rule`, `dnd.spring_load_ms`) as the drag runs, so a change takes
 * effect on the next move. A file drag and a tab drag are different sessions and never run
 * together: a press waits while the tab drag has the pointer.
 */
export function FileDragProvider({ manager, children }: FileDragProviderProps) {
	const api = useTabsApi();
	const snapshot = useTabsSnapshot();
	const vfs = useVfsClient();
	const ops = useOps();
	const trash = useTrashActions();
	const commands = useFileCommands();
	const tabDrag = useTabDragSession();
	const separate = useSeparateSession();
	const rule = useSettings((settings) => settings.dnd.defaultActionRule);
	const springMs = useSettings((settings) => settings.dnd.springLoadMs);
	const [picker, setPicker] = useState<PickerRequest | null>(null);

	// The drag is made once; what it reads changes under it.
	const latest = useRef({ api, snapshot, vfs, ops, trash, commands, rule, springMs, manager });
	latest.current = { api, snapshot, vfs, ops, trash, commands, rule, springMs, manager };

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
		});
	});
	useEffect(() => () => drag.dispose(), [drag]);

	const value = useMemo<FileDragApi>(
		() => ({
			press: (press) => {
				// A tab drag, or a pane's grip, has the pointer: a file drag does not begin beside it.
				const busy = [tabDrag.store.getState().phase, separate.store.getState().phase].some(
					(phase) => phase === 'pending' || phase === 'dragging',
				);
				return busy ? false : drag.press(press);
			},
			deferMenu: (open) => drag.deferMenu(open),
			consumeClick: () => drag.session.consumeClick(),
		}),
		[drag, tabDrag, separate],
	);

	return (
		<FileDragContext.Provider value={value}>
			{children}
			<DragStack session={drag.session} />
			{picker && <ActionPicker request={picker} onDone={() => setPicker(null)} />}
		</FileDragContext.Provider>
	);
}
