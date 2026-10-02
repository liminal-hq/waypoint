// The Settings window's sections: the pages that exist in this build, in the order they are listed
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { SettingsSectionDef } from '@liminal-hq/waypoint-chrome/SettingsShell/types';
import { t } from '../i18n/messages';
import { AccessibilityPage } from './AccessibilityPage';
import { AppearancePage } from './AppearancePage';
import { DragDropPage } from './DragDropPage';
import { GeneralPage } from './GeneralPage';
import { IntegrationsPage } from './IntegrationsPage';
import { OperationsPage } from './OperationsPage';
import { PreviewsPage } from './PreviewsPage';

export type SectionId =
	'general' | 'appearance' | 'accessibility' | 'previews' | 'operations' | 'dnd' | 'integrations';

/**
 * A page that is not built yet is absent, not a placeholder: the other pages of SPEC 11
 * (Appearance, Transparency, Tabs & windows and the rest) arrive with the milestone each belongs to.
 */
export function settingsSections(): SettingsSectionDef[] {
	return [
		{ id: 'general', label: t('settings.section.general'), render: () => <GeneralPage /> },
		{ id: 'appearance', label: t('settings.section.appearance'), render: () => <AppearancePage /> },
		{
			id: 'accessibility',
			label: t('settings.section.accessibility'),
			render: () => <AccessibilityPage />,
		},
		{ id: 'previews', label: t('settings.section.previews'), render: () => <PreviewsPage /> },
		{ id: 'operations', label: t('settings.section.operations'), render: () => <OperationsPage /> },
		{ id: 'dnd', label: t('settings.section.dnd'), render: () => <DragDropPage /> },
		{
			id: 'integrations',
			label: t('settings.section.integrations'),
			render: () => <IntegrationsPage />,
		},
	];
}
