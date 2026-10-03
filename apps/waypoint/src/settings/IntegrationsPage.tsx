// The Integrations page: what Waypoint does outside its own windows, each switch off until enabled, and the Services panel
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ButtonRow } from '@liminal-hq/waypoint-chrome/SettingsShell/ButtonRow';
import { SettingsGroup } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsGroup';
import { SettingsSection } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsSection';
import { TextRow } from '@liminal-hq/waypoint-chrome/SettingsShell/TextRow';
import { ToggleRow } from '@liminal-hq/waypoint-chrome/SettingsShell/ToggleRow';
import type { Availability } from '@liminal-hq/waypoint-protocol/generated/Availability';
import { useCallback, useEffect, useState } from 'react';
import { t, tf } from '../i18n/messages';
import { DEFAULT_ACCELERATOR } from '../services/accelerator';
import type {
	CurrentFileManager,
	DefaultFileManagerClient,
	FileManagerAction,
} from '../services/defaultFileManagerClient';
import { ServicesPanel } from './ServicesPanel';
import { useSettingsEditor } from './SettingsEditor';

/**
 * What a row says about whether its integration can work: nothing while it can, the Services
 * panel's own reason while it cannot, and the page's note when what the system can do could not be
 * read (a switch is then off, not guessed at). While it is still being read the row stays off.
 */
function useReason(
	pick: (
		availability: NonNullable<ReturnType<typeof useSettingsEditor>['availability']>,
	) => Availability,
): string | undefined {
	const { availability, availabilityUnreadable } = useSettingsEditor();
	if (availability === null) return availabilityUnreadable ?? t('settings.loading');
	const found = pick(availability);
	return found.available ? undefined : (found.reason ?? t('settings.unavailable'));
}

/** What the system lets Waypoint do about being the default for folders, and who is default now. */
function useFileManager(client: DefaultFileManagerClient | null) {
	const [action, setAction] = useState<FileManagerAction | null>(null);
	const [current, setCurrent] = useState<CurrentFileManager | null>(null);
	const [failed, setFailed] = useState<string | null>(null);
	const refresh = useCallback(async () => {
		if (!client) return;
		setCurrent(await client.current().catch(() => null));
	}, [client]);
	useEffect(() => {
		if (!client) return;
		let live = true;
		client.action().then(
			(found) => live && setAction(found),
			(error: unknown) => {
				console.warn('could not read what the default file manager can do', error);
				if (live) setAction({ kind: 'unavailable', reason: t('settings.integrations.unreadable') });
			},
		);
		client.current().then(
			(found) => live && setCurrent(found),
			() => live && setCurrent(null),
		);
		return () => {
			live = false;
		};
	}, [client]);
	const make = useCallback(async () => {
		if (!client) return;
		setFailed(null);
		try {
			await client.make();
		} catch (error) {
			setFailed(error instanceof Error ? error.message : String(error));
			return;
		}
		await refresh();
	}, [client, refresh]);
	return { action, current, failed, make };
}

/** The line under the default file manager row: who is default now, or why it could not be changed. */
function fileManagerNote(
	current: CurrentFileManager | null,
	failed: string | null,
): string | undefined {
	if (failed) return tf('settings.integrations.fileManager.failed', { reason: failed });
	if (current?.isWaypoint) return t('settings.integrations.fileManager.state.default');
	if (current?.name)
		return tf('settings.integrations.fileManager.state.other', { name: current.name });
	return undefined;
}

/**
 * Settings → Integrations. Every integration is off until it is enabled (D118); each switch is
 * shown dimmed with the Services panel's reason where the system cannot do it, and the Services
 * panel below lists every service. The settings are Rust's: a switch shows what is in force.
 */
export function IntegrationsPage() {
	const { settings, errors, changeSettings, fileManager, availability } = useSettingsEditor();
	const wanted = settings.integrations;
	const notifications = useReason((a) => a.notifications);
	// Hidden, not dimmed, where the system cannot draw buttons (D118); the Services panel says why.
	// It stays hidden until what the system can do has been read.
	const showButtons = availability?.notificationActions.available === true;
	const progress = useReason((a) => a.launcherProgress);
	const sleep = useReason((a) => a.preventSleep);
	const service = useReason((a) => a.fileManagerService);
	const shortcut = useReason((a) => a.globalShortcut);
	const change = (
		key: Parameters<typeof changeSettings>[0],
		edit: (integrations: typeof wanted) => typeof wanted,
	) => changeSettings(key, (s) => ({ ...s, integrations: edit(s.integrations) }));
	const defaults = useFileManager(fileManager);
	// Windows has no file manager name to own; the row is for Linux, so it is left out there.
	const showService = defaults.action?.kind !== 'settings';
	const note = fileManagerNote(defaults.current, defaults.failed);
	return (
		<SettingsSection>
			<SettingsGroup title={t('settings.group.notifications')}>
				<ToggleRow
					label={t('settings.integrations.notifications.label')}
					description={t('settings.integrations.notifications.description')}
					unavailableReason={notifications}
					error={errors.notifications}
					checked={wanted.notifications}
					onChange={(value) => change('notifications', (i) => ({ ...i, notifications: value }))}
				/>
				{showButtons && (
					<ToggleRow
						label={t('settings.integrations.notificationActions.label')}
						description={t('settings.integrations.notificationActions.description')}
						disabled={!wanted.notifications}
						error={errors.notificationActions}
						checked={wanted.notificationActions}
						onChange={(value) =>
							change('notificationActions', (i) => ({ ...i, notificationActions: value }))
						}
					/>
				)}
				<ToggleRow
					label={t('settings.integrations.progress.label')}
					description={t('settings.integrations.progress.description')}
					unavailableReason={progress}
					error={errors.launcherProgress}
					checked={wanted.launcherProgress}
					onChange={(value) =>
						change('launcherProgress', (i) => ({ ...i, launcherProgress: value }))
					}
				/>
				<ToggleRow
					label={t('settings.integrations.sleep.label')}
					description={t('settings.integrations.sleep.description')}
					unavailableReason={sleep}
					error={errors.preventSleep}
					checked={wanted.preventSleep}
					onChange={(value) => change('preventSleep', (i) => ({ ...i, preventSleep: value }))}
				/>
			</SettingsGroup>
			{defaults.action && (
				<SettingsGroup title={t('settings.group.fileManager')}>
					<ButtonRow
						label={
							defaults.action.kind === 'settings'
								? t('settings.integrations.fileManager.settings.label')
								: t('settings.integrations.fileManager.make.label')
						}
						description={
							<>
								{defaults.action.kind === 'settings'
									? t('settings.integrations.fileManager.settings.description')
									: t('settings.integrations.fileManager.make.description')}
								{note && (
									<>
										<br />
										<span role="status">{note}</span>
									</>
								)}
							</>
						}
						unavailableReason={
							defaults.action.kind === 'unavailable' ? defaults.action.reason : undefined
						}
						actionLabel={
							defaults.action.kind === 'settings'
								? t('settings.integrations.fileManager.settings.action')
								: t('settings.integrations.fileManager.make.action')
						}
						onAction={() => void defaults.make()}
					/>
					{showService && (
						<ToggleRow
							label={t('settings.integrations.fileManager.service.label')}
							description={t('settings.integrations.fileManager.service.description')}
							unavailableReason={service}
							error={errors.fileManagerService}
							checked={wanted.defaultFileManager}
							onChange={(value) =>
								change('fileManagerService', (i) => ({ ...i, defaultFileManager: value }))
							}
						/>
					)}
				</SettingsGroup>
			)}
			<SettingsGroup title={t('settings.group.shortcut')}>
				<ToggleRow
					label={t('settings.integrations.shortcut.enabled.label')}
					description={t('settings.integrations.shortcut.enabled.description')}
					unavailableReason={shortcut}
					error={errors.shortcutEnabled}
					checked={wanted.globalShortcutEnabled}
					onChange={(value) =>
						change('shortcutEnabled', (i) => ({ ...i, globalShortcutEnabled: value }))
					}
				/>
				<TextRow
					label={t('settings.integrations.shortcut.key.label')}
					description={t('settings.integrations.shortcut.key.description')}
					unavailableReason={shortcut}
					disabled={!wanted.globalShortcutEnabled}
					error={errors.shortcut}
					value={wanted.globalShortcut ?? DEFAULT_ACCELERATOR}
					placeholder={DEFAULT_ACCELERATOR}
					size={22}
					onCommit={(text) =>
						change('shortcut', (i) => ({
							...i,
							// An emptied field goes back to the default.
							globalShortcut: text.trim() === '' ? null : text.trim(),
						}))
					}
				/>
			</SettingsGroup>
			<ServicesPanel />
		</SettingsSection>
	);
}
