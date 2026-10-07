// The Transparency page: the switch, an opacity for each part of the window, blur, menus, a reset and a live preview
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { featureMessage, hasFeature } from '@liminal-hq/plugin-window-effects';
import { ButtonRow } from '@liminal-hq/waypoint-chrome/SettingsShell/ButtonRow';
import { SegmentedRow } from '@liminal-hq/waypoint-chrome/SettingsShell/SegmentedRow';
import { SettingsGroup } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsGroup';
import { SettingsSection } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsSection';
import { SliderRow } from '@liminal-hq/waypoint-chrome/SettingsShell/SliderRow';
import { ToggleRow } from '@liminal-hq/waypoint-chrome/SettingsShell/ToggleRow';
import { useEffect, useMemo, useState } from 'react';
import { t, tf, type MessageId } from '../i18n/messages';
import { useNativeContextMenusSetting } from '../menus/nativeMenuSetting';
import { MENU_OPACITY_MIN, OPACITY_MAX, OPACITY_MIN } from '../services/settingsClient';
import {
	effectiveAlphas,
	BLUR_OPACITY,
	blurInForce,
	blurShows,
	type Region,
} from '../theme/transparency';
import { useRootData, useSurfaceColours } from './rootLook';
import { useSettingsEditor } from './SettingsEditor';
import styles from './TransparencyPage.module.css';
import { TransparencyPreview } from './TransparencyPreview';
import { resetTransparency, transparencyAtDefaults } from './transparencyReset';

/** What to say when transparency is switched on and still not drawing, keyed by the reason ThemeRoot wrote on the root. */
const OFF_REASONS: Record<string, MessageId> = {
	'high-contrast': 'settings.transparency.off.highContrast',
	'reduced-transparency': 'settings.transparency.off.reducedTransparency',
	unavailable: 'settings.transparency.off.unavailable',
};

/** The setting each region's slider edits, the switch that turns the region translucent, and its words. */
const OPACITY_ROWS = [
	{
		region: 'titleBar',
		key: 'opacity',
		row: 'opacity',
		label: 'settings.transparency.opacity.label',
		description: 'settings.transparency.opacity.description',
		// The title bar's switch also governs the tabs and toolbar, but this slider is never dimmed by it.
		switchedBy: null,
	},
	{
		region: 'rows',
		key: 'rowsOpacity',
		row: 'rowsOpacity',
		label: 'settings.transparency.opacity.rows.label',
		description: 'settings.transparency.opacity.rows.description',
		switchedBy: 'titleBar',
	},
	{
		region: 'sidebar',
		key: 'sidebarOpacity',
		row: 'sidebarOpacity',
		label: 'settings.transparency.opacity.sidebar.label',
		description: 'settings.transparency.opacity.sidebar.description',
		switchedBy: 'sidebar',
	},
	{
		region: 'content',
		key: 'contentOpacity',
		row: 'contentOpacity',
		label: 'settings.transparency.opacity.content.label',
		description: 'settings.transparency.opacity.content.description',
		switchedBy: 'content',
	},
] as const satisfies readonly {
	region: Region;
	key: 'opacity' | 'rowsOpacity' | 'sidebarOpacity' | 'contentOpacity';
	row: 'opacity' | 'rowsOpacity' | 'sidebarOpacity' | 'contentOpacity';
	label: MessageId;
	description: MessageId;
	switchedBy: 'titleBar' | 'sidebar' | 'content' | null;
}[];

type OpacityKey = (typeof OPACITY_ROWS)[number]['key'];

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
	const nativeMenus = useNativeContextMenusSetting();
	const colours = useSurfaceColours();
	const reason = useRootData('transparencyReason');
	// The values a slider holds while it is dragged, before they are saved.
	const [drag, setDrag] = useState<Partial<Record<OpacityKey, number>>>({});
	const [dragMenuOpacity, setDragMenuOpacity] = useState<number | null>(null);
	// A value that is saved, reset or imported ends any drag the preview was still following.
	useEffect(() => {
		setDrag({});
		setDragMenuOpacity(null);
	}, [
		transparency.opacity,
		transparency.rowsOpacity,
		transparency.sidebarOpacity,
		transparency.contentOpacity,
		transparency.menuOpacity,
	]);

	const known = windowEffects !== null;
	const blurFeature =
		known &&
		(hasFeature(windowEffects, 'blur') ||
			hasFeature(windowEffects, 'mica') ||
			hasFeature(windowEffects, 'acrylic'));
	// A saved level only counts where the system can blur; elsewhere the parts keep their own opacities.
	const blur = blurInForce(transparency.blur, blurFeature);

	const effective = useMemo(
		() =>
			effectiveAlphas(
				{
					opacity: drag.opacity ?? transparency.opacity,
					rowsOpacity: drag.rowsOpacity ?? transparency.rowsOpacity,
					sidebarOpacity: drag.sidebarOpacity ?? transparency.sidebarOpacity,
					contentOpacity: drag.contentOpacity ?? transparency.contentOpacity,
					blur,
					regions: transparency.regions,
					menus: transparency.menus,
					menuOpacity: dragMenuOpacity ?? transparency.menuOpacity,
				},
				colours,
			),
		[drag, dragMenuOpacity, transparency, blur, colours],
	);
	// While the blur shows, the parts of the window are drawn at `BLUR_OPACITY` and their own opacity and the switch that draws an unfocused window solid wait.
	const blurred = blurShows(blur);

	const canBeSeeThrough = known && hasFeature(windowEffects, 'opacity');
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
				{blurFeature ? (
					<SegmentedRow
						label={t('settings.transparency.blur.label')}
						description={t('settings.transparency.blur.description')}
						error={errors.blur}
						value={transparency.blur}
						disabled={off}
						options={[
							{ value: 'off', label: t('settings.transparency.blur.off') },
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
			<SettingsGroup title={t('settings.group.transparencyOpacity')}>
				{OPACITY_ROWS.map(({ region, key, row, label, description, switchedBy }) => (
					<SliderRow
						key={key}
						label={t(label)}
						description={
							<>
								{t(description)}
								{blurred && (
									<span className={styles.note}>
										{tf('settings.transparency.opacity.byBlur', { percent: BLUR_OPACITY })}
									</span>
								)}
								{effective.raised[region] && (
									<span className={styles.note}>
										{tf('settings.transparency.opacity.raised', {
											percent: Math.round(effective.alphas[region] * 100),
										})}
									</span>
								)}
							</>
						}
						error={errors[row]}
						value={blurred ? BLUR_OPACITY : transparency[key]}
						min={OPACITY_MIN}
						max={OPACITY_MAX}
						unit={t('settings.transparency.opacity.unit')}
						disabled={off || blurred || (switchedBy !== null && !transparency.regions[switchedBy])}
						onInput={(value) => setDrag((current) => ({ ...current, [key]: value }))}
						onChange={(value) => {
							setDrag((current) => ({ ...current, [key]: undefined }));
							changeSettings(row, (s) => ({
								...s,
								transparency: { ...s.transparency, [key]: value },
							}));
						}}
					/>
				))}
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
						description={
							<>
								{t(`settings.transparency.regions.${region}.description`)}
								{blurred && (
									<span className={styles.note}>{t('settings.transparency.regions.byBlur')}</span>
								)}
							</>
						}
						error={errors[key]}
						checked={transparency.regions[region] || blurred}
						disabled={off || blurred}
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
					description={
						<>
							{t('settings.transparency.menus.description')}
							{nativeMenus && (
								<span className={styles.note}>{t('settings.transparency.menus.nativeNote')}</span>
							)}
						</>
					}
					error={errors.menus}
					checked={transparency.menus}
					disabled={off}
					onChange={(menus) =>
						changeSettings('menus', (s) => ({ ...s, transparency: { ...s.transparency, menus } }))
					}
				/>
				<SliderRow
					label={t('settings.transparency.menuOpacity.label')}
					description={
						<>
							{t('settings.transparency.menuOpacity.description')}
							{nativeMenus && (
								<span className={styles.note}>{t('settings.transparency.menus.nativeNote')}</span>
							)}
						</>
					}
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
					description={
						<>
							{t('settings.transparency.solidUnfocused.description')}
							{blurred && (
								<span className={styles.note}>
									{t('settings.transparency.solidUnfocused.byBlur')}
								</span>
							)}
							{/* Always laid out, so the note appearing never moves the rows below it. */}
							<span
								className={styles.reservedNote}
								role="status"
								data-active={transparency.solidWhenUnfocused && !blurred && reason === 'unfocused'}
							>
								{t('settings.transparency.off.unfocused')}
							</span>
						</>
					}
					error={errors.solidUnfocused}
					checked={transparency.solidWhenUnfocused && !blurred}
					disabled={off || blurred}
					onChange={(solidWhenUnfocused) =>
						changeSettings('solidUnfocused', (s) => ({
							...s,
							transparency: { ...s.transparency, solidWhenUnfocused },
						}))
					}
				/>
			</SettingsGroup>
			<SettingsGroup title={t('settings.group.transparencyReset')}>
				<ButtonRow
					label={t('settings.transparency.reset.label')}
					description={t('settings.transparency.reset.description')}
					error={errors.resetTransparency}
					actionLabel={t('settings.transparency.reset.action')}
					disabled={transparencyAtDefaults(transparency)}
					onAction={() => {
						setDrag({});
						setDragMenuOpacity(null);
						changeSettings('resetTransparency', (s) => ({
							...s,
							transparency: resetTransparency(s.transparency),
						}));
					}}
				/>
			</SettingsGroup>
		</SettingsSection>
	);
}
