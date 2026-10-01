// The Settings window's sections: the pages that exist in this build, in the order they are listed
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { SettingsSectionDef } from '@liminal-hq/waypoint-chrome/SettingsShell/types';
import { t } from '../i18n/messages';
import { DragDropPage } from './DragDropPage';
import { GeneralPage } from './GeneralPage';
import { OperationsPage } from './OperationsPage';

export type SectionId = 'general' | 'operations' | 'dnd';

/**
 * A page that is not built yet is absent, not a placeholder: the other pages of SPEC 11
 * (Appearance, Transparency, Tabs & windows and the rest) arrive with the milestone each belongs to.
 */
export function settingsSections(): SettingsSectionDef[] {
	return [
		{ id: 'general', label: t('settings.section.general'), render: () => <GeneralPage /> },
		{ id: 'operations', label: t('settings.section.operations'), render: () => <OperationsPage /> },
		{ id: 'dnd', label: t('settings.section.dnd'), render: () => <DragDropPage /> },
	];
}
