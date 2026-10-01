// The Drag & drop page: the default drop action, the spring-load delay and the Shelf
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { NumberRow } from '@liminal-hq/waypoint-chrome/SettingsShell/NumberRow';
import { SelectRow } from '@liminal-hq/waypoint-chrome/SettingsShell/SelectRow';
import { SettingsGroup } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsGroup';
import { SettingsSection } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsSection';
import { ToggleRow } from '@liminal-hq/waypoint-chrome/SettingsShell/ToggleRow';
import { t, tf } from '../i18n/messages';
import { SPRING_LOAD_MAX_MS, SPRING_LOAD_MIN_MS } from '../services/settingsClient';
import { useSettingsEditor } from './SettingsEditor';

export function DragDropPage() {
	const { settings, dnd, errors, changeSettings } = useSettingsEditor();
	const unavailable = dnd !== null && !dnd.outbound.available;
	return (
		<SettingsSection
			description={
				unavailable
					? tf('settings.dnd.unavailable', {
							reason: dnd.outbound.reason ?? t('settings.dnd.unavailable.noReason'),
						})
					: undefined
			}
		>
			<SettingsGroup title={t('settings.group.dropping')}>
				<SelectRow
					label={t('settings.dnd.rule.label')}
					description={t('settings.dnd.rule.description')}
					error={errors.dropRule}
					value={settings.dnd.defaultActionRule}
					options={[
						{ value: 'byVolume', label: t('settings.dnd.rule.byVolume') },
						{ value: 'alwaysCopy', label: t('settings.dnd.rule.alwaysCopy') },
						{ value: 'alwaysAsk', label: t('settings.dnd.rule.alwaysAsk') },
					]}
					onChange={(defaultActionRule) =>
						changeSettings('dropRule', (s) => ({ ...s, dnd: { ...s.dnd, defaultActionRule } }))
					}
				/>
				<NumberRow
					label={t('settings.dnd.spring.label')}
					description={t('settings.dnd.spring.description')}
					error={errors.springLoad}
					value={settings.dnd.springLoadMs}
					min={SPRING_LOAD_MIN_MS}
					max={SPRING_LOAD_MAX_MS}
					step={50}
					unit={t('settings.dnd.spring.unit')}
					commitOn="commit"
					onChange={(springLoadMs) =>
						changeSettings('springLoad', (s) => ({ ...s, dnd: { ...s.dnd, springLoadMs } }))
					}
				/>
			</SettingsGroup>
			<SettingsGroup title={t('settings.group.shelf')}>
				<ToggleRow
					label={t('settings.dnd.shelf.label')}
					description={t('settings.dnd.shelf.description')}
					error={errors.shelfPersist}
					checked={settings.dnd.shelfPersist}
					onChange={(shelfPersist) =>
						changeSettings('shelfPersist', (s) => ({ ...s, dnd: { ...s.dnd, shelfPersist } }))
					}
				/>
			</SettingsGroup>
		</SettingsSection>
	);
}
