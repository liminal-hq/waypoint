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
import { SettingsShell } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsShell';
import { WindowFrame } from '@liminal-hq/waypoint-chrome/WindowFrame';
import { useMemo, useState } from 'react';
import { useLocaleVersion } from '../i18n/active';
import { t } from '../i18n/messages';
import { createTauriOpsClient } from '../services/tauriOpsClient';
import { createTauriSettingsClient } from '../services/tauriSettingsClient';
import type { SettingsClient } from '../services/settingsClient';
import { SettingsProvider, useSettingsHandle } from '../settings/SettingsContext';
import {
	SettingsEditorProvider,
	useSettingsEditor,
	type DndAvailability,
	type OpsSettingsApi,
} from '../settings/SettingsEditor';
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
}

/** The native drag and drop plugin's status in the shape the page needs. */
async function nativeDndAvailability(): Promise<DndAvailability> {
	const { outbound } = (await nativeDndStatus()).features;
	return { outbound: { available: outbound.available, reason: outbound.reason } };
}

function Pages() {
	const { ready } = useSettingsEditor();
	const [active, setActive] = useState<SectionId>('general');
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
}: Pick<SettingsScreenProps, 'ops' | 'dndStatus' | 'thumbnailsStatus' | 'windowEffectsStatus'>) {
	const handle = useSettingsHandle();
	const [ownOps] = useState<OpsSettingsApi>(() => ops ?? createTauriOpsClient());
	if (!handle) return null;
	return (
		<SettingsEditorProvider
			handle={handle}
			ops={ownOps}
			dndStatus={dndStatus ?? nativeDndAvailability}
			thumbnailsStatus={thumbnailsStatus ?? thumbnailsPluginStatus}
			windowEffectsStatus={windowEffectsStatus ?? windowEffectsPluginStatus}
		>
			<Pages />
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
					/>
				</SettingsProvider>
			</main>
			<NoticeToast />
		</WindowFrame>
	);
}
