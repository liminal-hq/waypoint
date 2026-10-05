// The Settings window: the shared settings shell over the settings plugin and the operations plugin's own settings
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { getStatus as nativeDndStatus } from '@liminal-hq/plugin-native-dnd';
import {
	getStatus as windowEffectsPluginStatus,
	type PluginStatus as WindowEffectsStatus,
} from '@liminal-hq/plugin-window-effects';
import {
	getStatus as thumbnailsPluginStatus,
	type PluginStatus,
} from '@liminal-hq/plugin-thumbnails';
import { connectionSupport } from '@liminal-hq/waypoint-plugin-vfs';
import { SettingsShell } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsShell';
import { WindowFrame } from '@liminal-hq/waypoint-chrome/WindowFrame';
import { listen } from '@tauri-apps/api/event';
import { useEffect, useMemo, useState } from 'react';
import { useLocaleVersion } from '../i18n/active';
import { t } from '../i18n/messages';
import { createTauriConnectionsClient } from '../connections/tauriConnectionsClient';
import type { DefaultFileManagerClient } from '../services/defaultFileManagerClient';
import type { IntegrationsClient } from '../services/integrationsClient';
import { createTauriDefaultFileManagerClient } from '../services/tauriDefaultFileManagerClient';
import { createTauriIntegrationsClient } from '../services/tauriIntegrationsClient';
import { createTauriOpsClient } from '../services/tauriOpsClient';
import { createTauriSettingsClient } from '../services/tauriSettingsClient';
import { createTauriSettingsTransferClient } from '../services/tauriSettingsTransferClient';
import type { SettingsClient } from '../services/settingsClient';
import type { SettingsTransferClient } from '../services/settingsTransferClient';
import { SettingsProvider, useSettingsHandle } from '../settings/SettingsContext';
import {
	SettingsEditorProvider,
	useSettingsEditor,
	type DndAvailability,
	type OpsSettingsApi,
	type ProtocolSupport,
} from '../settings/SettingsEditor';
import {
	createServerPreviewsClient,
	ServerPreviewsProvider,
	type ServerPreviewsClient,
} from '../settings/ServerPreviews';
import { settingsSections, type SectionId } from '../settings/settingsSections';
import { AppTitleBar } from './AppTitleBar';
import { NoticeToast } from './NoticeToast';
import styles from './SettingsScreen.module.css';

interface SettingsScreenProps {
	/** The settings plugin's client; the real one unless a test supplies its own. */
	client?: SettingsClient;
	/** The operations plugin's settings commands; the real ones unless a test supplies its own. */
	ops?: OpsSettingsApi;
	/** What drag and drop can do here; the native plugin's status unless a test supplies its own. */
	dndStatus?: () => Promise<DndAvailability>;
	/** What thumbnails can do here; the thumbnails plugin's status unless a test supplies its own. */
	thumbnailsStatus?: () => Promise<PluginStatus>;
	/** What window effects can do here; the window effects plugin's status unless a test supplies its own. */
	windowEffectsStatus?: () => Promise<WindowEffectsStatus>;
	/** What the OS integrations can do here; the app's own commands unless a test supplies its own. */
	integrations?: IntegrationsClient;
	/** Which remote protocols this build has; the file system plugin's unless a test supplies its own. */
	protocolSupport?: () => Promise<ProtocolSupport>;
	/** The page to open on (`experimental`); the window's `?section=` unless a test supplies its own. */
	initialSection?: string;
	/** The default file manager action; the mime-apps plugin's unless a test supplies its own. */
	fileManager?: DefaultFileManagerClient;
	/** Export and import of the settings; the settings plugin's unless a test supplies its own. */
	transfer?: SettingsTransferClient;
	/** The saved connections' previews choices; the connections plugin's unless a test supplies its own (`null` for none). */
	servers?: ServerPreviewsClient | null;
}

/** The native drag and drop plugin's status in the shape the page needs. */
async function nativeDndAvailability(): Promise<DndAvailability> {
	const { outbound } = (await nativeDndStatus()).features;
	return { outbound: { available: outbound.available, reason: outbound.reason } };
}

/** What the Settings window shows when it opens: the page a link asked for, or the first. */
function sectionFromUrl(): string | null {
	try {
		return new URLSearchParams(window.location.search).get('section');
	} catch {
		return null;
	}
}

/** The event the app sends an open Settings window to make it show another page (`settings_window.rs`). */
const SECTION_EVENT = 'waypoint://settings-section';

function Pages({ initialSection }: { initialSection?: string | undefined }) {
	const { ready } = useSettingsEditor();
	const [active, setActive] = useState<SectionId>(
		() => (initialSection ?? sectionFromUrl() ?? 'general') as SectionId,
	);
	// A link from another window (an address of a protocol that is off) shows its page here.
	useEffect(() => {
		let stop: (() => void) | undefined;
		let live = true;
		listen<string>(SECTION_EVENT, (event) => setActive(event.payload as SectionId)).then(
			(unlisten) => (live ? (stop = unlisten) : unlisten()),
			() => {},
		);
		return () => {
			live = false;
			stop?.();
		};
	}, []);
	// A new language re-renders the pages in place, so the page the person is on stays open.
	const version = useLocaleVersion();
	// eslint-disable-next-line react-hooks/exhaustive-deps
	const sections = useMemo(() => settingsSections(), [version]);
	if (!ready) {
		return (
			<p role="status" className={styles.loading}>
				{t('settings.loading')}
			</p>
		);
	}
	return (
		<SettingsShell
			sections={sections}
			activeId={active}
			onSelect={(id) => setActive(id as SectionId)}
			labels={{ navigation: t('settings.nav.label'), unavailable: t('settings.unavailable') }}
		/>
	);
}

function Editor({
	ops,
	dndStatus,
	thumbnailsStatus,
	windowEffectsStatus,
	integrations,
	protocolSupport,
	initialSection,
	fileManager,
	transfer,
	servers,
}: Pick<
	SettingsScreenProps,
	| 'ops'
	| 'dndStatus'
	| 'thumbnailsStatus'
	| 'windowEffectsStatus'
	| 'integrations'
	| 'protocolSupport'
	| 'initialSection'
	| 'fileManager'
	| 'transfer'
	| 'servers'
>) {
	const handle = useSettingsHandle();
	const [ownOps] = useState<OpsSettingsApi>(() => ops ?? createTauriOpsClient());
	const [ownIntegrations] = useState(() => integrations ?? createTauriIntegrationsClient());
	const [ownFileManager] = useState(() => fileManager ?? createTauriDefaultFileManagerClient());
	const [ownTransfer] = useState(() => transfer ?? createTauriSettingsTransferClient());
	const [ownServers] = useState(() =>
		servers === undefined ? createServerPreviewsClient(createTauriConnectionsClient()) : servers,
	);
	if (!handle) return null;
	return (
		<SettingsEditorProvider
			handle={handle}
			ops={ownOps}
			dndStatus={dndStatus ?? nativeDndAvailability}
			thumbnailsStatus={thumbnailsStatus ?? thumbnailsPluginStatus}
			windowEffectsStatus={windowEffectsStatus ?? windowEffectsPluginStatus}
			integrations={ownIntegrations}
			protocolSupport={protocolSupport ?? connectionSupport}
			fileManager={ownFileManager}
			transfer={ownTransfer}
		>
			<ServerPreviewsProvider client={ownServers}>
				<Pages initialSection={initialSection} />
			</ServerPreviewsProvider>
		</SettingsEditorProvider>
	);
}

/**
 * Edits the settings. Changes apply at once (there is no Save button): each one is asked of Rust,
 * and a row shows what Rust says is in force, with a refusal under it. Both documents are edited
 * through their owners, the settings plugin and the operations plugin.
 */
export function SettingsScreen({
	client,
	ops,
	dndStatus,
	thumbnailsStatus,
	windowEffectsStatus,
	integrations,
	protocolSupport,
	initialSection,
	fileManager,
	transfer,
	servers,
}: SettingsScreenProps) {
	const [own] = useState(() => client ?? createTauriSettingsClient());
	// The title bar and the pages render their messages themselves, so a new language needs a render.
	useLocaleVersion();
	return (
		<WindowFrame className={styles.screen}>
			<AppTitleBar title={t('window.settings.title')} />
			<main className={styles.content}>
				<SettingsProvider client={own}>
					<Editor
						ops={ops}
						dndStatus={dndStatus}
						thumbnailsStatus={thumbnailsStatus}
						windowEffectsStatus={windowEffectsStatus}
						integrations={integrations}
						protocolSupport={protocolSupport}
						initialSection={initialSection}
						fileManager={fileManager}
						transfer={transfer}
						servers={servers}
					/>
				</SettingsProvider>
			</main>
			<NoticeToast />
		</WindowFrame>
	);
}
