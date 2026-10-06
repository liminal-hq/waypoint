// The Appearance page: colour mode, theme source, accent, density, the icon theme, folder colour and icon style
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ColourRow } from '@liminal-hq/waypoint-chrome/SettingsShell/ColourRow';
import { SegmentedRow } from '@liminal-hq/waypoint-chrome/SettingsShell/SegmentedRow';
import { SelectRow } from '@liminal-hq/waypoint-chrome/SettingsShell/SelectRow';
import { ToggleRow } from '@liminal-hq/waypoint-chrome/SettingsShell/ToggleRow';
import { SettingsGroup } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsGroup';
import { SettingsSection } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsSection';
import type { AccentChoice } from '@liminal-hq/waypoint-protocol/generated/AccentChoice';
import { formatLocale } from '../i18n/active';
import { t, tf } from '../i18n/messages';
import styles from './AppearancePage.module.css';
import { resolveIconTheme } from '../icons/iconTheme';
import { useSystemOffer } from '../icons/systemIcons';
import { EMBER } from '../theme/accent';
import type { LiftedColour } from '../theme/palette';
import { usePaletteReport } from '../theme/paletteReport';
import { FolderColourRow, IconStyleRow, IconThemeRow } from './IconChoices';
import { useSettingsEditor } from './SettingsEditor';

/** What the contrast floor moved, in words: one phrase per kind of colour, in a fixed order. */
function liftedParts(lifted: readonly LiftedColour[]): string {
	const has = (...tokens: string[]): boolean => lifted.some((one) => tokens.includes(one.token));
	const parts = [
		has('text-primary', 'text-secondary', 'text-muted') &&
			t('settings.appearance.systemColours.part.text'),
		has('bg-selected') && t('settings.appearance.systemColours.part.selection'),
		has('danger', 'success', 'warning') && t('settings.appearance.systemColours.part.status'),
		has('focus-ring') && t('settings.appearance.systemColours.part.focus'),
		has(
			'title-bar-top',
			'title-bar-bottom',
			'title-bar-top-unfocused',
			'title-bar-bottom-unfocused',
		) && t('settings.appearance.systemColours.part.titleBar'),
	].filter((part): part is string => part !== false);
	return new Intl.ListFormat(formatLocale(), { style: 'long', type: 'conjunction' }).format(parts);
}

/** The colour the picker starts at when a person chooses "Choose a colour". */
const CUSTOM_START = EMBER.light.fill;

export function AppearancePage() {
	const { settings, errors, changeSettings } = useSettingsEditor();
	const { appearance } = settings;
	const accent = appearance.accent;
	const offer = useSystemOffer();
	const chosen = resolveIconTheme(appearance.iconTheme);
	// A setting of `system` the system cannot supply draws the Waypoint icons, so the page shows that;
	// until the plugin has answered, the choice stands.
	const system = chosen === 'system' && (offer.loading || offer.offered);
	const iconTheme = chosen === 'system' && !system ? 'waypoint' : chosen;
	const portage = iconTheme === 'portage';
	// Offered only where the system has a palette (the Services panel says why elsewhere); hidden
	// until the theme has found out.
	const palette = usePaletteReport();
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
				{palette.available === true && (
					<>
						<ToggleRow
							label={t('settings.appearance.systemColours.label')}
							description={t('settings.appearance.systemColours.description')}
							error={errors.matchSystemColours}
							checked={appearance.matchSystemColours}
							onChange={(matchSystemColours) =>
								changeSettings('matchSystemColours', (s) => ({
									...s,
									appearance: { ...s.appearance, matchSystemColours },
								}))
							}
						/>
						{palette.state === 'applied' && palette.lifted.length > 0 && (
							<p className={styles.note} role="status">
								{tf('settings.appearance.systemColours.lifted', {
									parts: liftedParts(palette.lifted),
								})}
							</p>
						)}
						{palette.state === 'high-contrast' && (
							<p className={styles.note}>{t('settings.appearance.systemColours.highContrast')}</p>
						)}
						{palette.state === 'other-variant' && (
							<p className={styles.note}>{t('settings.appearance.systemColours.otherVariant')}</p>
						)}
					</>
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
				{!offer.loading && !offer.offered && (
					<p className={styles.note}>
						{offer.reason
							? tf('settings.appearance.iconTheme.systemUnavailable', { reason: offer.reason })
							: t('settings.appearance.iconTheme.systemUnavailableNoReason')}
					</p>
				)}
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
					<p className={styles.note}>
						{t(
							system
								? 'settings.appearance.folderColour.systemNote'
								: 'settings.appearance.folderColour.waypointNote',
						)}
					</p>
				)}
				<IconStyleRow
					disabled={portage || system}
					error={errors.iconStyle}
					value={appearance.iconStyle}
					onChange={(iconStyle) =>
						changeSettings('iconStyle', (s) => ({
							...s,
							appearance: { ...s.appearance, iconStyle },
						}))
					}
				/>
				{system && <p className={styles.note}>{t('settings.appearance.iconStyle.systemNote')}</p>}
			</SettingsGroup>
		</SettingsSection>
	);
}
