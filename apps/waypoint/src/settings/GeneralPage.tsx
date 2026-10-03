// The General page: start-up, how new windows browse, and whether the Trash asks first
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { SegmentedRow } from '@liminal-hq/waypoint-chrome/SettingsShell/SegmentedRow';
import { SettingsGroup } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsGroup';
import { SettingsSection } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsSection';
import { ToggleRow } from '@liminal-hq/waypoint-chrome/SettingsShell/ToggleRow';
import { t, tf } from '../i18n/messages';
import { BackupGroup } from './BackupGroup';
import { DEFAULT_OPS } from './opsDefaults';
import { useSettingsEditor } from './SettingsEditor';

export function GeneralPage() {
	const { settings, ops, opsUnreadable, errors, changeSettings, changeOps } = useSettingsEditor();
	const { general } = settings;
	return (
		<SettingsSection>
			<SettingsGroup title={t('settings.group.startup')}>
				<SegmentedRow
					label={t('settings.general.startup.label')}
					description={t('settings.general.startup.description')}
					error={errors.startup}
					value={general.startup}
					options={[
						{ value: 'restoreSession', label: t('settings.general.startup.restore') },
						{ value: 'home', label: t('settings.general.startup.home') },
					]}
					onChange={(startup) =>
						changeSettings('startup', (s) => ({ ...s, general: { ...s.general, startup } }))
					}
				/>
			</SettingsGroup>
			<SettingsGroup title={t('settings.group.browsing')}>
				<SegmentedRow
					label={t('settings.general.defaultView.label')}
					description={t('settings.general.defaultView.description')}
					error={errors.defaultView}
					value={general.defaultView}
					options={[
						{ value: 'list', label: t('settings.general.defaultView.list') },
						{ value: 'grid', label: t('settings.general.defaultView.grid') },
					]}
					onChange={(defaultView) =>
						changeSettings('defaultView', (s) => ({ ...s, general: { ...s.general, defaultView } }))
					}
				/>
				<SegmentedRow
					label={t('settings.general.clickMode.label')}
					description={t('settings.general.clickMode.description')}
					error={errors.clickMode}
					value={general.clickMode}
					options={[
						{ value: 'single', label: t('settings.general.clickMode.single') },
						{ value: 'double', label: t('settings.general.clickMode.double') },
					]}
					onChange={(clickMode) =>
						changeSettings('clickMode', (s) => ({ ...s, general: { ...s.general, clickMode } }))
					}
				/>
				<ToggleRow
					label={t('settings.general.rememberFolderViews.label')}
					description={t('settings.general.rememberFolderViews.description')}
					error={errors.rememberFolderViews}
					checked={general.rememberFolderViews}
					onChange={(rememberFolderViews) =>
						changeSettings('rememberFolderViews', (s) => ({
							...s,
							general: { ...s.general, rememberFolderViews },
						}))
					}
				/>
				<ToggleRow
					label={t('settings.general.showHidden.label')}
					description={t('settings.general.showHidden.description')}
					error={errors.showHidden}
					checked={general.showHiddenDefault}
					onChange={(showHiddenDefault) =>
						changeSettings('showHidden', (s) => ({
							...s,
							general: { ...s.general, showHiddenDefault },
						}))
					}
				/>
			</SettingsGroup>
			<SettingsGroup title={t('settings.group.titleBar')}>
				<ToggleRow
					label={t('settings.general.appNameInTitle.label')}
					description={t('settings.general.appNameInTitle.description')}
					error={errors.appNameInTitle}
					checked={settings.ui.appNameInTitle}
					onChange={(appNameInTitle) =>
						changeSettings('appNameInTitle', (s) => ({ ...s, ui: { ...s.ui, appNameInTitle } }))
					}
				/>
				<ToggleRow
					label={t('settings.general.menuBar.label')}
					description={t('settings.general.menuBar.description')}
					error={errors.menuBar}
					checked={settings.ui.menuBar}
					onChange={(menuBar) =>
						changeSettings('menuBar', (s) => ({ ...s, ui: { ...s.ui, menuBar } }))
					}
				/>
			</SettingsGroup>
			<SettingsGroup title={t('settings.group.deleting')}>
				<ToggleRow
					label={t('settings.general.confirmTrash.label')}
					description={
						opsUnreadable
							? tf('settings.ops.unreadable', { reason: opsUnreadable })
							: t('settings.general.confirmTrash.description')
					}
					error={errors.confirmTrash}
					disabled={ops === null}
					checked={(ops ?? DEFAULT_OPS).confirmTrash}
					onChange={(confirmTrash) => changeOps('confirmTrash', (o) => ({ ...o, confirmTrash }))}
				/>
			</SettingsGroup>
			<BackupGroup />
		</SettingsSection>
	);
}
