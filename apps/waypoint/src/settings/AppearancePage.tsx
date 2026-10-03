// The Appearance page: colour mode, theme source, accent, density, the icon theme, folder colour and icon style
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ColourRow } from '@liminal-hq/waypoint-chrome/SettingsShell/ColourRow';
import { SegmentedRow } from '@liminal-hq/waypoint-chrome/SettingsShell/SegmentedRow';
import { SelectRow } from '@liminal-hq/waypoint-chrome/SettingsShell/SelectRow';
import { SettingsGroup } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsGroup';
import { SettingsSection } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsSection';
import type { AccentChoice } from '@liminal-hq/waypoint-protocol/generated/AccentChoice';
import { t } from '../i18n/messages';
import styles from './AppearancePage.module.css';
import { resolveIconTheme } from '../icons/iconTheme';
import { EMBER } from '../theme/accent';
import { FolderColourRow, IconThemeRow } from './IconChoices';
import { useSettingsEditor } from './SettingsEditor';

/** The colour the picker starts at when a person chooses "Choose a colour". */
const CUSTOM_START = EMBER.light.fill;

export function AppearancePage() {
	const { settings, errors, changeSettings } = useSettingsEditor();
	const { appearance } = settings;
	const accent = appearance.accent;
	// `system` is in the schema but not offered yet, and draws as Waypoint, so the page shows it so.
	const iconTheme = resolveIconTheme(appearance.iconTheme);
	const portage = iconTheme === 'portage';
	return (
		<SettingsSection>
			<SettingsGroup title={t('settings.group.colours')}>
				<SegmentedRow
					label={t('settings.appearance.mode.label')}
					description={t('settings.appearance.mode.description')}
					error={errors.mode}
					value={appearance.mode}
					options={[
						{ value: 'system', label: t('settings.appearance.mode.system') },
						{ value: 'light', label: t('settings.appearance.mode.light') },
						{ value: 'dark', label: t('settings.appearance.mode.dark') },
					]}
					onChange={(mode) =>
						changeSettings('mode', (s) => ({ ...s, appearance: { ...s.appearance, mode } }))
					}
				/>
				<SegmentedRow
					label={t('settings.appearance.source.label')}
					description={t('settings.appearance.source.description')}
					error={errors.themeSource}
					value={appearance.themeSource}
					options={[
						{ value: 'liminal', label: t('settings.appearance.source.liminal') },
						{ value: 'os', label: t('settings.appearance.source.os') },
					]}
					onChange={(themeSource) =>
						changeSettings('themeSource', (s) => ({
							...s,
							appearance: { ...s.appearance, themeSource },
						}))
					}
				/>
				<SelectRow
					label={t('settings.appearance.accent.label')}
					description={t('settings.appearance.accent.description')}
					error={errors.accent}
					value={accent.kind}
					options={[
						{ value: 'ember', label: t('settings.appearance.accent.ember') },
						{ value: 'os', label: t('settings.appearance.accent.os') },
						{ value: 'custom', label: t('settings.appearance.accent.custom') },
					]}
					onChange={(kind) =>
						changeSettings('accent', (s) => {
							const next: AccentChoice =
								kind === 'custom'
									? {
											kind,
											hex:
												s.appearance.accent.kind === 'custom'
													? s.appearance.accent.hex
													: CUSTOM_START,
										}
									: { kind };
							return { ...s, appearance: { ...s.appearance, accent: next } };
						})
					}
				/>
				{accent.kind === 'custom' && (
					<ColourRow
						label={t('settings.appearance.accentColour.label')}
						description={t('settings.appearance.accentColour.description')}
						error={errors.accentColour}
						value={accent.hex}
						onChange={(hex) =>
							changeSettings('accentColour', (s) => ({
								...s,
								appearance: { ...s.appearance, accent: { kind: 'custom', hex } },
							}))
						}
					/>
				)}
			</SettingsGroup>
			<SettingsGroup title={t('settings.group.layout')}>
				<SegmentedRow
					label={t('settings.appearance.density.label')}
					description={t('settings.appearance.density.description')}
					error={errors.density}
					value={appearance.density}
					options={[
						{ value: 'compact', label: t('settings.appearance.density.compact') },
						{ value: 'comfortable', label: t('settings.appearance.density.comfortable') },
						{ value: 'spacious', label: t('settings.appearance.density.spacious') },
					]}
					onChange={(density) =>
						changeSettings('density', (s) => ({ ...s, appearance: { ...s.appearance, density } }))
					}
				/>
			</SettingsGroup>
			<SettingsGroup title={t('settings.group.icons')}>
				<IconThemeRow
					error={errors.iconTheme}
					value={iconTheme}
					folderColour={appearance.folderColour}
					onChange={(next) =>
						changeSettings('iconTheme', (s) => ({
							...s,
							appearance: { ...s.appearance, iconTheme: next },
						}))
					}
				/>
				{portage ? (
					<FolderColourRow
						error={errors.folderColour}
						value={appearance.folderColour}
						onChange={(folderColour) =>
							changeSettings('folderColour', (s) => ({
								...s,
								appearance: { ...s.appearance, folderColour },
							}))
						}
					/>
				) : (
					<p className={styles.note}>{t('settings.appearance.folderColour.waypointNote')}</p>
				)}
				<SelectRow
					label={t('settings.appearance.iconStyle.label')}
					description={t('settings.appearance.iconStyle.description')}
					disabled={portage}
					error={errors.iconStyle}
					value={appearance.iconStyle}
					options={[
						{ value: 'light', label: t('settings.appearance.iconStyle.light') },
						{ value: 'regular', label: t('settings.appearance.iconStyle.regular') },
						{ value: 'bold', label: t('settings.appearance.iconStyle.bold') },
						{ value: 'filled', label: t('settings.appearance.iconStyle.filled') },
					]}
					onChange={(iconStyle) =>
						changeSettings('iconStyle', (s) => ({
							...s,
							appearance: { ...s.appearance, iconStyle },
						}))
					}
				/>
			</SettingsGroup>
		</SettingsSection>
	);
}
