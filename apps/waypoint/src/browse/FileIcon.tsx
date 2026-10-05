// The icon for an entry in the icon set the window is set to: a Waypoint glyph or Portage art
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { IconGroup } from '@liminal-hq/waypoint-protocol/generated/IconGroup';
import type { SpecialFolder } from '@liminal-hq/waypoint-protocol/generated/SpecialFolder';
import { useIconLook, type ResolvedIconTheme } from '../icons/iconTheme';
import type { EntryIconSource } from '../icons/systemIconTarget';
import type { PortageFolderBadge } from '../icons/portage/portageFolderArt';
import type { FolderColour, FolderTone } from '../icons/portage/portagePalette';
import { cssUrl, glyphSvg, pictureUrl } from '../icons/iconPictures';
import { PortageIcon } from '../icons/PortageIcon';
import { portageIconSvg } from '../icons/portageIcons';
import { WAYPOINT_FILE_GLYPHS, WAYPOINT_FOLDER_GLYPHS } from '../icons/waypointFileIcons';
import styles from './FileIcon.module.css';
import { useIconPictures } from './IconPictures';
import { SystemIcon } from './SystemIcon';

interface FileIconProps {
	group: IconGroup;
	/** Which standard folder of the user's this is, for a folder that is one: it gets its own mark. */
	special?: SpecialFolder | null;
	/** The entry's name, for the System set: its extension picks the type's icon. The other sets go by the group. */
	name?: string;
	/** The entry the icon is for, for the System set: a program or a shortcut is drawn from its own icon. */
	source?: EntryIconSource;
	/** The size the icon is drawn at in CSS pixels (default 16), so the System set asks for a picture no bigger than it needs. */
	size?: number;
	/** An extra class, for a view that draws the glyph larger than the list does. */
	className?: string;
	/** Draw in this set whatever the window is set to: the Settings previews show each option. */
	theme?: ResolvedIconTheme;
	/** The Portage folder colour to draw, instead of the setting. */
	colour?: FolderColour;
	/** The Portage tone to draw, instead of the window's. */
	tone?: FolderTone;
	/** A sticker the Portage set draws on a folder (the Git mark of a repository); the other sets draw none. */
	badge?: PortageFolderBadge | undefined;
}

/**
 * The icon for an entry's group (a folder may carry a badge in the Portage set). Decorative: the row's name carries the meaning. The set follows the
 * window's icon theme live (Portage in the chosen folder colour and the window's tone, the Waypoint
 * glyphs, or the icons the system draws for the entry's type, with the Waypoint glyph while they load
 * and wherever the system has none); `className` sizes any of them.
 */
export function FileIcon({
	group,
	special,
	name,
	source,
	size = 16,
	className,
	badge,
	...override
}: FileIconProps) {
	const look = useIconLook();
	const pictures = useIconPictures();
	const theme = override.theme ?? look.theme;
	const colour = override.colour ?? look.colour;
	const tone = override.tone ?? look.tone;
	const marked = group === 'folder' && special ? special : undefined;
	if (pictures) {
		// Under `IconPictures` (the grid): the same art as a shared picture, not an inline SVG.
		const picture = (svg: string, system?: boolean) => (
			<span
				className={className ? `${styles.picture} ${className}` : styles.picture}
				data-group={group}
				data-special={marked}
				data-system={system ? '' : undefined}
				data-icon-picture=""
				aria-hidden="true"
				style={{ backgroundImage: cssUrl(pictureUrl(svg)) }}
			/>
		);
		if (theme === 'portage') {
			return picture(portageIconSvg({ group, special, colour, tone, badge }));
		}
		const key = marked ? `folder:${marked}` : group;
		const glyph = picture(
			glyphSvg(
				key,
				marked ? WAYPOINT_FOLDER_GLYPHS[marked] : WAYPOINT_FILE_GLYPHS[group],
				group === 'folder' ? pictures.folder : pictures.file,
				size,
			),
		);
		if (theme !== 'system') return glyph;
		return (
			<SystemIcon
				group={group}
				special={special}
				name={name}
				source={source}
				size={size}
				tone={tone}
				className={className}
				fallback={glyph}
				picture
			/>
		);
	}
	if (theme === 'portage') {
		return (
			<PortageIcon
				group={group}
				special={special}
				colour={colour}
				tone={tone}
				badge={badge}
				className={className}
			/>
		);
	}
	const glyph = (
		<svg
			className={className ? `${styles.icon} ${className}` : styles.icon}
			data-group={group}
			data-special={marked}
			viewBox="0 0 16 16"
			aria-hidden="true"
			focusable="false"
		>
			{marked ? WAYPOINT_FOLDER_GLYPHS[marked] : WAYPOINT_FILE_GLYPHS[group]}
		</svg>
	);
	if (theme === 'system') {
		return (
			<SystemIcon
				group={group}
				special={special}
				name={name}
				source={source}
				size={size}
				tone={tone}
				className={className}
				fallback={glyph}
			/>
		);
	}
	return glyph;
}
