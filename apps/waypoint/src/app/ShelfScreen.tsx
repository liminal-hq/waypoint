// The Shelf window: the Shelf on its own, over the same session and the same panel as the dock
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { getHome } from '@liminal-hq/waypoint-plugin-vfs';
import { WindowFrame } from '@liminal-hq/waypoint-chrome/WindowFrame';
import {
	useWindowControls,
	WindowChromeProvider,
} from '@liminal-hq/waypoint-chrome/WindowChromeProvider/WindowChromeProvider';
import type { WindowControls } from '@liminal-hq/waypoint-chrome/TitleBar/windowControls';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { useCallback, useEffect, useMemo, useState, type ReactNode } from 'react';
import { ListingManager } from '../browse/listingManager';
import { useVfsClient, VfsClientProvider } from '../browse/VfsClientContext';
import { CommandBridgeProvider, createCommandBridge } from '../commands/commandBridge';
import { FileDragProvider } from '../dnd/FileDragContext';
import { t } from '../i18n/messages';
import { ClipboardProvider } from '../ops/ClipboardContext';
import { FileCommandsProvider } from '../ops/FileCommandsContext';
import { OpsProvider } from '../ops/OpsContext';
import { startOpsAnnouncer } from '../ops/opsAnnouncer';
import type { OpsHandle } from '../ops/opsStore';
import { currentWindowLabel } from '../ops/windowLabel';
import { useFileCommandsHost } from '../ops/useFileCommandsHost';
import type { NativeDndClient } from '../services/nativeDndClient';
import type { OpsClient } from '../services/opsClient';
import type { OsClipboardClient } from '../services/osClipboardClient';
import type { ShelfWindowClient } from '../services/shelfWindowClient';
import { tabsApi as realTabsApi, type TabsApi } from '../services/tabsApi';
import { createTauriNativeDndClient } from '../services/tauriNativeDndClient';
import { createTauriOpsClient } from '../services/tauriOpsClient';
import { createTauriOsClipboardClient } from '../services/tauriOsClipboardClient';
import { createTauriVfsClient } from '../services/tauriVfsClient';
import type { VfsClient } from '../services/vfsClient';
import { ShelfProvider } from '../shelf/ShelfContext';
import { ShelfPanel } from '../shelf/ShelfPanel';
import { TabDragProvider } from '../tabs/TabDragContext';
import { announce, useAnnouncement } from '../tabs/announcer';
import { TabsProvider } from '../tabs/TabsContext';
import { AppTitleBar } from './AppTitleBar';
import { useWindowTitle } from '@liminal-hq/waypoint-chrome/WindowTitle/useWindowTitle';
import { NoticeToast } from './NoticeToast';
import styles from './ShelfScreen.module.css';
import { WindowCommands } from './WindowCommands';

/** What the Shelf window runs on: the same plugins as a main window, without its tabs, panes or sidebar. */
export interface ShelfScreenServices {
	/** Where the session's tabs would open; the Shelf window has none, but the session provider asks. */
	home: Location;
	vfs: VfsClient;
	tabsApi: TabsApi;
	ops?: OpsClient;
	osClipboard?: OsClipboardClient;
	/** Files dragged in from other applications and the Shelf's drags out of the window. */
	nativeDnd?: NativeDndClient;
	/** Raises and hides this window; the real one unless a test supplies its own. */
	windowClient?: ShelfWindowClient;
}

/** The real services. */
export async function startShelfServices(): Promise<ShelfScreenServices> {
	return {
		home: await getHome(),
		vfs: createTauriVfsClient(),
		tabsApi: realTabsApi,
		ops: createTauriOpsClient(),
		osClipboard: createTauriOsClipboardClient(),
		nativeDnd: createTauriNativeDndClient(),
	};
}

/**
 * Remembers whether the window stays on top. The title bar's pin (and the Always on Top command)
 * ask the window controls, so the controls it sees are the real ones plus a note to the session;
 * a change the window manager makes itself is noted too. Where the system cannot keep a window on
 * top the title bar shows no pin, so nothing is asked.
 */
function RememberOnTop({ api, children }: { api: TabsApi; children: ReactNode }) {
	const base = useWindowControls();
	const remember = useCallback(
		(on: boolean) => {
			api.setShelfOnTop(on).catch((error: unknown) => {
				console.warn('could not remember the Shelf window staying on top', error);
			});
		},
		[api],
	);
	const controls = useMemo<WindowControls>(
		() => ({
			...base,
			setAlwaysOnTop: (on) => {
				remember(on);
				return base.setAlwaysOnTop(on);
			},
		}),
		[base, remember],
	);
	useEffect(() => {
		const stop = base.onAlwaysOnTopChange?.(remember);
		return () => stop?.();
	}, [base, remember]);
	return <WindowChromeProvider controls={controls}>{children}</WindowChromeProvider>;
}

/**
 * The panel and what it needs from the window around it: the file drag (drops onto the Shelf and
 * drags of its items out), the clipboard, and the Shelf's own provider with `inWindow` set. It
 * has no listings of its own, so its manager holds none; the Shelf's own references are what it
 * lists.
 */
function ShelfWindowBody({
	nativeDnd,
	windowClient,
}: {
	nativeDnd?: NativeDndClient | undefined;
	windowClient?: ShelfWindowClient | undefined;
}) {
	const vfs = useVfsClient();
	const [manager] = useState(() => new ListingManager(vfs, {}));
	useEffect(() => () => manager.dispose(), [manager]);
	const { commands, dialog, clipboard } = useFileCommandsHost(noSession);
	return (
		<FileCommandsProvider value={commands}>
			<ClipboardProvider value={clipboard}>
				<ShelfProvider
					activeSession={noSession}
					inWindow
					{...(windowClient ? { windowClient } : {})}
				>
					<FileDragProvider manager={manager} nativeDnd={nativeDnd}>
						<ShelfPanel layout="window" />
					</FileDragProvider>
				</ShelfProvider>
			</ClipboardProvider>
			{dialog}
		</FileCommandsProvider>
	);
}

const noSession = () => null;

/**
 * The window shows the Shelf's items, groups and selection state the way the dock does, because
 * it is the same panel over the same session: every window follows the one store in Rust through
 * its events, and the Shelf window is one more of them. Closing it docks the Shelf again.
 */
export function ShelfScreen({ services }: { services?: ShelfScreenServices }) {
	const [bridge] = useState(createCommandBridge);
	const [started, setStarted] = useState<ShelfScreenServices | null>(services ?? null);
	const [failure, setFailure] = useState<string | null>(null);
	useEffect(() => {
		if (services) return;
		let active = true;
		startShelfServices().then(
			(value) => active && setStarted(value),
			(error: unknown) =>
				active && setFailure(error instanceof Error ? error.message : String(error)),
		);
		return () => {
			active = false;
		};
	}, [services]);
	useWindowTitle(t('window.shelf.osTitle'));
	const announcement = useAnnouncement();
	const onHandle = useCallback((handle: OpsHandle) => startOpsAnnouncer(handle, { announce }), []);
	const windowLabel = useMemo(currentWindowLabel, []);

	return (
		<CommandBridgeProvider value={bridge}>
			<WindowFrame className={styles.screen}>
				<WindowCommands />
				{started && (
					<RememberOnTop api={started.tabsApi}>
						<AppTitleBar title={t('window.shelf.title')} />
					</RememberOnTop>
				)}
				{!started && <AppTitleBar title={t('window.shelf.title')} />}
				<main className={styles.content}>
					{/* Shown while files from another window are held over this one: the whole window is the target. */}
					<div className={styles.dropCue} aria-hidden="true">
						<span className={styles.dropHint}>{t('window.shelf.dropHint')}</span>
					</div>
					{failure !== null && (
						<p role="alert" className={styles.failure}>
							{t('window.shelf.startFailed')} <span data-selectable="">{failure}</span>
						</p>
					)}
					{started && (
						<VfsClientProvider client={started.vfs}>
							<TabsProvider api={started.tabsApi} home={started.home}>
								<TabDragProvider>
									{started.ops ? (
										<OpsProvider
											client={started.ops}
											windowLabel={windowLabel}
											osClipboard={started.osClipboard ?? null}
											onHandle={onHandle}
										>
											<ShelfWindowBody
												nativeDnd={started.nativeDnd}
												windowClient={started.windowClient}
											/>
										</OpsProvider>
									) : (
										<ShelfWindowBody
											nativeDnd={started.nativeDnd}
											windowClient={started.windowClient}
										/>
									)}
								</TabDragProvider>
							</TabsProvider>
						</VfsClientProvider>
					)}
				</main>
				<NoticeToast />
				<div className={styles.srOnly} role="status" aria-live="polite">
					{announcement}
				</div>
			</WindowFrame>
		</CommandBridgeProvider>
	);
}
