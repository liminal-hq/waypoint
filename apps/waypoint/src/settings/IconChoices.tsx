// The Appearance page's icon rows: the icon theme and icon style pickers with live previews, and the folder colour swatches
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useRef, type KeyboardEvent, type ReactNode } from 'react';
import {
	SettingsRow,
	useSettingsRowControl,
	type SettingsRowProps,
} from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsRow';
import type { IconGroup } from '@liminal-hq/waypoint-protocol/generated/IconGroup';
import { FileIcon } from '../browse/FileIcon';
import { t } from '../i18n/messages';
import type { IconStyle } from '@liminal-hq/waypoint-protocol/generated/IconStyle';
import { HomeIcon, StarIcon } from '../icons/AppIcons';
import { TrashIcon } from '../icons/MenuIcons';
import { useIconLook, type ResolvedIconTheme } from '../icons/iconTheme';
import { FOLDER_COLOURS, type FolderColour } from '../icons/portage/portagePalette';
import { PortageIcon } from '../icons/PortageIcon';
import styles from './IconChoices.module.css';

/** The sample entries each theme draws in its preview: a folder, an image, a PDF, a document and an archive. */
export const PREVIEW_GROUPS: readonly IconGroup[] = [
	'folder',
	'image',
	'pdf',
	'document',
	'archive',
];

/** The icon styles the setting offers, in the order the page lists them. */
export const ICON_STYLES: readonly IconStyle[] = ['light', 'regular', 'bold', 'filled'];

interface Choice<T extends string> {
	value: T;
	/** The visible name. It is also the radio's accessible name, so no choice is told by colour alone. */
	label: string;
	preview: ReactNode;
}

interface ChoiceGroupProps<T extends string> {
	value: T;
	choices: Choice<T>[];
	onChange: (value: T) => void;
	layout: 'cards' | 'swatches';
}

/** A radio group of buttons with arrow-key, Home and End movement and one tab stop. */
function ChoiceGroup<T extends string>({ value, choices, onChange, layout }: ChoiceGroupProps<T>) {
	const { controlId, labelId, describedBy, disabled } = useSettingsRowControl();
	const group = useRef<HTMLDivElement>(null);
	const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
		const rtl = group.current ? getComputedStyle(group.current).direction === 'rtl' : false;
		const forward = rtl ? 'ArrowLeft' : 'ArrowRight';
		const back = rtl ? 'ArrowRight' : 'ArrowLeft';
		const current = choices.findIndex((choice) => choice.value === value);
		let next = -1;
		if (event.key === forward || event.key === 'ArrowDown') next = (current + 1) % choices.length;
		else if (event.key === back || event.key === 'ArrowUp')
			next = (current - 1 + choices.length) % choices.length;
		else if (event.key === 'Home') next = 0;
		else if (event.key === 'End') next = choices.length - 1;
		const target = choices[next];
		if (next < 0 || !target) return;
		event.preventDefault();
		onChange(target.value);
		group.current
			?.querySelector<HTMLElement>(`[data-value="${CSS.escape(target.value)}"]`)
			?.focus();
	};
	return (
		<div
			ref={group}
			id={controlId}
			role="radiogroup"
			aria-labelledby={labelId}
			aria-describedby={describedBy}
			aria-disabled={disabled || undefined}
			className={layout === 'cards' ? styles.cards : styles.swatches}
			onKeyDown={onKeyDown}
		>
			{choices.map((choice) => {
				const checked = choice.value === value;
				return (
					<button
						key={choice.value}
						type="button"
						role="radio"
						aria-checked={checked}
						data-value={choice.value}
						tabIndex={checked ? 0 : -1}
						disabled={disabled}
						title={choice.label}
						className={`${layout === 'cards' ? styles.card : styles.swatch} ${checked ? styles.on : ''}`}
						onClick={() => onChange(choice.value)}
					>
						{choice.preview}
						<span className={styles.name}>
							{checked && (
								<svg
									className={styles.check}
									viewBox="0 0 16 16"
									aria-hidden="true"
									focusable="false"
								>
									<path d="M3.5 8.5l3 3 6-6.5" />
								</svg>
							)}
							{choice.label}
						</span>
					</button>
				);
			})}
		</div>
	);
}

/** The five sample icons as one theme draws them (the colour and tone of the window's setting). */
function PreviewStrip({ theme, colour }: { theme: ResolvedIconTheme; colour: FolderColour }) {
	const { tone } = useIconLook();
	return (
		<span className={styles.strip} data-theme-preview={theme}>
			{PREVIEW_GROUPS.map((group) => (
				<FileIcon
					key={group}
					group={group}
					theme={theme}
					colour={colour}
					tone={tone}
					className={styles.sample}
				/>
			))}
		</span>
	);
}

interface IconThemeRowProps extends Omit<SettingsRowProps, 'children' | 'label' | 'description'> {
	/** `system` is not offered yet, so the value the page holds is only ever one of these two. */
	value: ResolvedIconTheme;
	folderColour: FolderColour;
	onChange: (value: ResolvedIconTheme) => void;
}

/** The icon theme picker: one card per theme with a preview strip drawn by that theme. */
export function IconThemeRow({ value, folderColour, onChange, ...row }: IconThemeRowProps) {
	const themes: ResolvedIconTheme[] = ['waypoint', 'portage'];
	return (
		<SettingsRow
			label={t('settings.appearance.iconTheme.label')}
			description={t('settings.appearance.iconTheme.description')}
			{...row}
		>
			<ChoiceGroup
				layout="cards"
				value={value}
				onChange={onChange}
				choices={themes.map((theme) => ({
					value: theme,
					label: t(`settings.appearance.iconTheme.${theme}`),
					preview: <PreviewStrip theme={theme} colour={folderColour} />,
				}))}
			/>
		</SettingsRow>
	);
}

interface FolderColourRowProps extends Omit<
	SettingsRowProps,
	'children' | 'label' | 'description'
> {
	value: FolderColour;
	onChange: (value: FolderColour) => void;
}

/** The ten folder colours as swatches: a Portage folder in each colour, with its name under it. */
export function FolderColourRow({ value, onChange, ...row }: FolderColourRowProps) {
	const { tone } = useIconLook();
	return (
		<SettingsRow
			label={t('settings.appearance.folderColour.label')}
			description={t('settings.appearance.folderColour.description')}
			{...row}
		>
			<ChoiceGroup
				layout="swatches"
				value={value}
				onChange={onChange}
				choices={FOLDER_COLOURS.map((colour) => ({
					value: colour,
					label: t(`settings.appearance.folderColour.${colour}`),
					preview: (
						<PortageIcon
							group="folder"
							colour={colour}
							tone={tone}
							size={32}
							className={styles.chip}
						/>
					),
				}))}
			/>
		</SettingsRow>
	);
}

/**
 * One style's sample strip: a folder, a file and three toolbar and menu glyphs. `data-icon-style-preview`
 * gives the strip that style's weight and fill whatever the window is set to (`theme/tokens.css`), and
 * the Waypoint set is drawn even where the window shows Portage, because the style applies to it alone.
 */
function StyleStrip({ style }: { style: IconStyle }) {
	return (
		<span className={styles.strip} data-icon-style-preview={style}>
			<FileIcon group="folder" theme="waypoint" className={styles.sample} />
			<FileIcon group="document" theme="waypoint" className={styles.sample} />
			<HomeIcon className={styles.sample} />
			<StarIcon className={styles.sample} />
			<TrashIcon className={styles.sample} />
		</span>
	);
}

interface IconStyleRowProps extends Omit<SettingsRowProps, 'children' | 'label' | 'description'> {
	value: IconStyle;
	onChange: (value: IconStyle) => void;
}

/** The icon style picker: one card per style with a strip of icons drawn in that style, so the choice shows what it does. */
export function IconStyleRow({ value, onChange, ...row }: IconStyleRowProps) {
	return (
		<SettingsRow
			label={t('settings.appearance.iconStyle.label')}
			description={t('settings.appearance.iconStyle.description')}
			{...row}
		>
			<ChoiceGroup
				layout="cards"
				value={value}
				onChange={onChange}
				choices={ICON_STYLES.map((style) => ({
					value: style,
					label: t(`settings.appearance.iconStyle.${style}`),
					preview: <StyleStrip style={style} />,
				}))}
			/>
		</SettingsRow>
	);
}
