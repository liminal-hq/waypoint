// The icon for an entry in the icon set the window is set to: a Waypoint glyph or Portage art
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { IconGroup } from '@liminal-hq/waypoint-protocol/generated/IconGroup';
import type { SpecialFolder } from '@liminal-hq/waypoint-protocol/generated/SpecialFolder';
import { useIconLook, type ResolvedIconTheme } from '../icons/iconTheme';
import type { PortageFolderBadge } from '../icons/portage/portageFolderArt';
import type { FolderColour, FolderTone } from '../icons/portage/portagePalette';
import { PortageIcon } from '../icons/PortageIcon';
import { WAYPOINT_FILE_GLYPHS, WAYPOINT_FOLDER_GLYPHS } from '../icons/waypointFileIcons';
import styles from './FileIcon.module.css';
import { SystemIcon } from './SystemIcon';

interface FileIconProps {
	group: IconGroup;
	/** Which standard folder of the user's this is, for a folder that is one: it gets its own mark. */
	special?: SpecialFolder | null;
	/** The entry's name, for the System set: its extension picks the type's icon. The other sets go by the group. */
	name?: string;
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
	size = 16,
	className,
	badge,
	...override
}: FileIconProps) {
	const look = useIconLook();
	const theme = override.theme ?? look.theme;
	const colour = override.colour ?? look.colour;
	const tone = override.tone ?? look.tone;
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
	const marked = group === 'folder' && special ? special : undefined;
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
				size={size}
				tone={tone}
				className={className}
				fallback={glyph}
			/>
		);
	}
	return glyph;
}
