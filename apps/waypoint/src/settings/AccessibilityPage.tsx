// The Accessibility page: high contrast, text size, motion, transparency, touch mode and the focus ring
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { SegmentedRow } from '@liminal-hq/waypoint-chrome/SettingsShell/SegmentedRow';
import { SelectRow } from '@liminal-hq/waypoint-chrome/SettingsShell/SelectRow';
import { SettingsGroup } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsGroup';
import { SettingsSection } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsSection';
import { ToggleRow } from '@liminal-hq/waypoint-chrome/SettingsShell/ToggleRow';
import type { OsPreference } from '@liminal-hq/waypoint-protocol/generated/OsPreference';
import { t } from '../i18n/messages';
import { useSettingsEditor, type RowKey } from './SettingsEditor';

const FOLLOW_OPTIONS = (): { value: OsPreference; label: string }[] => [
	{ value: 'follow', label: t('settings.access.follow') },
	{ value: 'on', label: t('settings.access.on') },
	{ value: 'off', label: t('settings.access.off') },
];

export function AccessibilityPage() {
	const { settings, errors, changeSettings } = useSettingsEditor();
	const access = settings.accessibility;
	const follow = (
		key: RowKey,
		field: 'highContrast' | 'reducedMotion' | 'reducedTransparency',
		label: string,
		description: string,
	) => (
		<SelectRow
			label={label}
			description={description}
			error={errors[key]}
			value={access[field]}
			options={FOLLOW_OPTIONS()}
			onChange={(value) =>
				changeSettings(key, (s) => ({
					...s,
					accessibility: { ...s.accessibility, [field]: value },
				}))
			}
		/>
	);
	return (
		<SettingsSection>
			<SettingsGroup title={t('settings.group.vision')}>
				{follow(
					'highContrast',
					'highContrast',
					t('settings.access.highContrast.label'),
					t('settings.access.highContrast.description'),
				)}
				<SegmentedRow
					label={t('settings.access.textSize.label')}
					description={t('settings.access.textSize.description')}
					error={errors.textSize}
					value={String(access.textSize) as '100' | '115' | '130'}
					options={[
						{ value: '100', label: t('settings.access.textSize.100') },
						{ value: '115', label: t('settings.access.textSize.115') },
						{ value: '130', label: t('settings.access.textSize.130') },
					]}
					onChange={(size) =>
						changeSettings('textSize', (s) => ({
							...s,
							accessibility: { ...s.accessibility, textSize: Number(size) },
						}))
					}
				/>
				<ToggleRow
					label={t('settings.access.strongFocus.label')}
					description={t('settings.access.strongFocus.description')}
					error={errors.strongFocus}
					checked={access.strongFocusRing}
					onChange={(strongFocusRing) =>
						changeSettings('strongFocus', (s) => ({
							...s,
							accessibility: { ...s.accessibility, strongFocusRing },
						}))
					}
				/>
			</SettingsGroup>
			<SettingsGroup title={t('settings.group.motion')}>
				{follow(
					'reducedMotion',
					'reducedMotion',
					t('settings.access.reducedMotion.label'),
					t('settings.access.reducedMotion.description'),
				)}
				{follow(
					'reducedTransparency',
					'reducedTransparency',
					t('settings.access.reducedTransparency.label'),
					t('settings.access.reducedTransparency.description'),
				)}
			</SettingsGroup>
			<SettingsGroup title={t('settings.group.touch')}>
				<SegmentedRow
					label={t('settings.access.touchMode.label')}
					description={t('settings.access.touchMode.description')}
					error={errors.touchMode}
					value={access.touchMode}
					options={[
						{ value: 'off', label: t('settings.access.touchMode.off') },
						{ value: 'auto', label: t('settings.access.touchMode.auto') },
						{ value: 'on', label: t('settings.access.touchMode.on') },
					]}
					onChange={(touchMode) =>
						changeSettings('touchMode', (s) => ({
							...s,
							accessibility: { ...s.accessibility, touchMode },
						}))
					}
				/>
			</SettingsGroup>
		</SettingsSection>
	);
}
