// The icon for an entry in the icon set the window is set to: a Waypoint glyph or Portage art
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { IconGroup } from '@liminal-hq/waypoint-protocol/generated/IconGroup';
import type { SpecialFolder } from '@liminal-hq/waypoint-protocol/generated/SpecialFolder';
import { useIconLook, type ResolvedIconTheme } from '../icons/iconTheme';
import type { FolderColour, FolderTone } from '../icons/portage/portagePalette';
import { PortageIcon } from '../icons/PortageIcon';
import { WAYPOINT_FILE_GLYPHS, WAYPOINT_FOLDER_GLYPHS } from '../icons/waypointFileIcons';
import styles from './FileIcon.module.css';

interface FileIconProps {
	group: IconGroup;
	/** Which standard folder of the user's this is, for a folder that is one: it gets its own mark. */
	special?: SpecialFolder | null;
	/** An extra class, for a view that draws the glyph larger than the list does. */
	className?: string;
	/** Draw in this set whatever the window is set to: the Settings previews show each option. */
	theme?: ResolvedIconTheme;
	/** The Portage folder colour to draw, instead of the setting. */
	colour?: FolderColour;
	/** The Portage tone to draw, instead of the window's. */
	tone?: FolderTone;
}

/**
 * The icon for an entry's group. Decorative: the row's name carries the meaning. The set follows the
 * window's icon theme live (Portage in the chosen folder colour and the window's tone, or the Waypoint
 * glyphs); `className` sizes either.
 */
export function FileIcon({ group, special, className, ...override }: FileIconProps) {
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
				className={className}
			/>
		);
	}
	const marked = group === 'folder' && special ? special : undefined;
	return (
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
}
