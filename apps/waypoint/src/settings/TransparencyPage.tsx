// The Transparency page: the master switch, window and menu opacity, blur, the parts of the window and a live preview
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { featureMessage, hasFeature } from '@liminal-hq/plugin-window-effects';
import { SegmentedRow } from '@liminal-hq/waypoint-chrome/SettingsShell/SegmentedRow';
import { SettingsGroup } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsGroup';
import { SettingsSection } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsSection';
import { SliderRow } from '@liminal-hq/waypoint-chrome/SettingsShell/SliderRow';
import { ToggleRow } from '@liminal-hq/waypoint-chrome/SettingsShell/ToggleRow';
import { useMemo, useState } from 'react';
import { t, tf, type MessageId } from '../i18n/messages';
import { MENU_OPACITY_MIN, OPACITY_MAX, OPACITY_MIN } from '../services/settingsClient';
import { effectiveAlphas } from '../theme/transparency';
import { useRootData, useSurfaceColours } from './rootLook';
import { useSettingsEditor } from './SettingsEditor';
import styles from './TransparencyPage.module.css';
import { TransparencyPreview } from './TransparencyPreview';

/** What to say when transparency is switched on and still not drawing, keyed by the reason ThemeRoot wrote on the root. */
const OFF_REASONS: Record<string, MessageId> = {
	'high-contrast': 'settings.transparency.off.highContrast',
	'reduced-transparency': 'settings.transparency.off.reducedTransparency',
	unavailable: 'settings.transparency.off.unavailable',
	unfocused: 'settings.transparency.off.unfocused',
};

/**
 * Transparency over the `transparency` settings. What the platform can do decides what is shown
 * (A60): where windows cannot be see-through at all the page says why and offers nothing; where
 * blur is missing (GNOME) its row is left out and the reason is shown instead. The sliders preview
 * while they are dragged, in the sample window and nowhere else, and save when they are let go.
 * The sample is drawn by the same function the real windows use, so what it shows is what they
 * will draw, the contrast floor included.
 */
export function TransparencyPage() {
	const { settings, windowEffects, errors, changeSettings } = useSettingsEditor();
	const transparency = settings.transparency;
	const colours = useSurfaceColours();
	const reason = useRootData('transparencyReason');
	// The values a slider holds while it is dragged, before they are saved.
	const [dragOpacity, setDragOpacity] = useState<number | null>(null);
	const [dragMenuOpacity, setDragMenuOpacity] = useState<number | null>(null);

	const effective = useMemo(
		() =>
			effectiveAlphas(
				{
					opacity: dragOpacity ?? transparency.opacity,
					regions: transparency.regions,
					menus: transparency.menus,
					menuOpacity: dragMenuOpacity ?? transparency.menuOpacity,
				},
				colours,
			),
		[dragOpacity, dragMenuOpacity, transparency, colours],
	);

	const known = windowEffects !== null;
	const canBeSeeThrough = known && hasFeature(windowEffects, 'opacity');
	const blurFeature =
		known &&
		(hasFeature(windowEffects, 'blur') ||
			hasFeature(windowEffects, 'mica') ||
			hasFeature(windowEffects, 'acrylic'));
	// Where windows are not translucent by default, the effect is the compositor's and WebKitGTK's to get right.
	const experimental =
		known && windowEffects.flavour !== 'windows' && windowEffects.flavour !== 'unsupported';

	if (!canBeSeeThrough) {
		return (
			<SettingsSection
				description={
					known
						? tf('settings.transparency.unavailable', {
								reason:
									featureMessage(windowEffects, 'opacity') ??
									windowEffects.message ??
									t('settings.previews.unavailable.noReason'),
							})
						: t('settings.transparency.unknown')
				}
			>
				{null}
			</SettingsSection>
		);
	}

	const off = !transparency.enabled;
	const raisedAlphas = (Object.keys(effective.alphas) as (keyof typeof effective.alphas)[])
		.filter((region) => effective.raised[region])
		.map((region) => effective.alphas[region]);
	const floorPercent = raisedAlphas.length ? Math.round(Math.min(...raisedAlphas) * 100) : null;
	const blurReason =
		featureMessage(windowEffects, 'blur') ?? t('settings.transparency.blur.unavailable.noReason');

	return (
		<SettingsSection>
			<SettingsGroup title={t('settings.group.transparencyPreview')}>
				<TransparencyPreview effective={effective} />
			</SettingsGroup>
			<SettingsGroup title={t('settings.group.transparencyWindow')}>
				<ToggleRow
					label={
						<>
							{t('settings.transparency.enable.label')}
							{experimental && (
								<span className={styles.badge}>{t('settings.transparency.experimental')}</span>
							)}
						</>
					}
					description={
						<>
							{t('settings.transparency.enable.description')}
							{experimental && (
								<span className={styles.note}>
									{t('settings.transparency.enable.experimental')}
								</span>
							)}
							{transparency.enabled && reason && OFF_REASONS[reason] && (
								<span className={styles.note} role="status">
									{t(OFF_REASONS[reason])}
								</span>
							)}
						</>
					}
					error={errors.transparency}
					checked={transparency.enabled}
					onChange={(enabled) =>
						changeSettings('transparency', (s) => ({
							...s,
							transparency: { ...s.transparency, enabled },
						}))
					}
				/>
				<SliderRow
					label={t('settings.transparency.opacity.label')}
					description={
						<>
							{t('settings.transparency.opacity.description')}
							{floorPercent !== null && (
								<span className={styles.note}>
									{tf('settings.transparency.opacity.raised', { percent: floorPercent })}
								</span>
							)}
						</>
					}
					error={errors.opacity}
					value={transparency.opacity}
					min={OPACITY_MIN}
					max={OPACITY_MAX}
					unit={t('settings.transparency.opacity.unit')}
					disabled={off}
					onInput={setDragOpacity}
					onChange={(opacity) => {
						setDragOpacity(null);
						changeSettings('opacity', (s) => ({
							...s,
							transparency: { ...s.transparency, opacity },
						}));
					}}
				/>
				{blurFeature ? (
					<SegmentedRow
						label={t('settings.transparency.blur.label')}
						description={t('settings.transparency.blur.description')}
						error={errors.blur}
						value={transparency.blur}
						disabled={off}
						options={[
							{ value: 'off', label: t('settings.transparency.blur.off') },
							{ value: 'low', label: t('settings.transparency.blur.low') },
							{ value: 'high', label: t('settings.transparency.blur.high') },
						]}
						onChange={(blur) =>
							changeSettings('blur', (s) => ({ ...s, transparency: { ...s.transparency, blur } }))
						}
					/>
				) : (
					<p className={styles.unavailable}>
						{tf('settings.transparency.blur.unavailable', { reason: blurReason })}
					</p>
				)}
			</SettingsGroup>
			<SettingsGroup title={t('settings.group.transparencyRegions')}>
				{(
					[
						['titleBar', 'regionTitleBar'],
						['sidebar', 'regionSidebar'],
						['content', 'regionContent'],
					] as const
				).map(([region, key]) => (
					<ToggleRow
						key={region}
						label={t(`settings.transparency.regions.${region}.label`)}
						description={t(`settings.transparency.regions.${region}.description`)}
						error={errors[key]}
						checked={transparency.regions[region]}
						disabled={off}
						onChange={(value) =>
							changeSettings(key, (s) => ({
								...s,
								transparency: {
									...s.transparency,
									regions: { ...s.transparency.regions, [region]: value },
								},
							}))
						}
					/>
				))}
			</SettingsGroup>
			<SettingsGroup title={t('settings.group.transparencyMenus')}>
				<ToggleRow
					label={t('settings.transparency.menus.label')}
					description={t('settings.transparency.menus.description')}
					error={errors.menus}
					checked={transparency.menus}
					disabled={off}
					onChange={(menus) =>
						changeSettings('menus', (s) => ({ ...s, transparency: { ...s.transparency, menus } }))
					}
				/>
				<SliderRow
					label={t('settings.transparency.menuOpacity.label')}
					description={t('settings.transparency.menuOpacity.description')}
					error={errors.menuOpacity}
					value={transparency.menuOpacity}
					min={MENU_OPACITY_MIN}
					max={OPACITY_MAX}
					unit={t('settings.transparency.opacity.unit')}
					disabled={off || !transparency.menus}
					onInput={setDragMenuOpacity}
					onChange={(menuOpacity) => {
						setDragMenuOpacity(null);
						changeSettings('menuOpacity', (s) => ({
							...s,
							transparency: { ...s.transparency, menuOpacity },
						}));
					}}
				/>
			</SettingsGroup>
			<SettingsGroup title={t('settings.group.transparencyFocus')}>
				<ToggleRow
					label={t('settings.transparency.solidUnfocused.label')}
					description={t('settings.transparency.solidUnfocused.description')}
					error={errors.solidUnfocused}
					checked={transparency.solidWhenUnfocused}
					disabled={off}
					onChange={(solidWhenUnfocused) =>
						changeSettings('solidUnfocused', (s) => ({
							...s,
							transparency: { ...s.transparency, solidWhenUnfocused },
						}))
					}
				/>
			</SettingsGroup>
		</SettingsSection>
	);
}
