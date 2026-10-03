// The icon for an entry, drawn from the Waypoint set: one small SVG glyph per icon group
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { IconGroup } from '@liminal-hq/waypoint-protocol/generated/IconGroup';
import type { SpecialFolder } from '@liminal-hq/waypoint-protocol/generated/SpecialFolder';
import { WAYPOINT_FILE_GLYPHS, WAYPOINT_FOLDER_GLYPHS } from '../icons/waypointFileIcons';
import styles from './FileIcon.module.css';

interface FileIconProps {
	group: IconGroup;
	/** Which standard folder of the user's this is, for a folder that is one: it gets its own mark. */
	special?: SpecialFolder | null;
	/** An extra class, for a view that draws the glyph larger than the list does. */
	className?: string;
}

/** The glyph for an entry's icon group. Decorative: the row's name carries the meaning. */
export function FileIcon({ group, special, className }: FileIconProps) {
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
