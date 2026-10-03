// The icon the operating system shows for an entry's type, over the Waypoint glyph until it has loaded and wherever the system has none
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useMemo, type ReactNode } from 'react';
import type { IconGroup } from '@liminal-hq/waypoint-protocol/generated/IconGroup';
import type { SpecialFolder } from '@liminal-hq/waypoint-protocol/generated/SpecialFolder';
import type { FolderTone } from '../icons/portage/portagePalette';
import { iconScale, iconSizeFor, systemIconTarget } from '../icons/systemIconTarget';
import {
	requestSystemImage,
	systemIconUrl,
	useSystemIcons,
	useSystemImage,
} from '../icons/systemIcons';
import styles from './FileIcon.module.css';

interface SystemIconProps {
	group: IconGroup;
	special?: SpecialFolder | null;
	/** The entry's name, whose extension picks the type's icon. Without it the group's stand-in type is drawn. */
	name?: string;
	/** The size the icon is drawn at in CSS pixels, so the picture asked for is no bigger than it needs to be. */
	size: number;
	tone: FolderTone;
	className?: string;
	/** What is drawn until the system's icon is there, and for good where the system has none. Never nothing. */
	fallback: ReactNode;
}

/**
 * The system's icon for the entry's type, as an SVG `<image>` in the same frame the Waypoint glyph has, so
 * every rule that sizes the glyph sizes this too. The icon is asked for by type (the extension, or the
 * group's stand-in type, or the kind of folder), so a listing of thousands of files asks for a handful of
 * pictures, and each address is loaded once and shared by every row with it. Decorative: the row's name is
 * the accessible name.
 */
export function SystemIcon({
	group,
	special,
	name,
	size,
	tone,
	className,
	fallback,
}: SystemIconProps) {
	const icons = useSystemIcons();
	const kind = group === 'folder' ? icons.folder : icons.type;
	const usable = icons.phase === 'ready' && kind.available;
	const extensionOrGroup = group === 'folder' ? null : name;
	const url = useMemo(
		() =>
			usable
				? systemIconUrl(
						systemIconTarget(group, extensionOrGroup ?? undefined, special),
						{ size: iconSizeFor(size), scale: iconScale(globalThis.devicePixelRatio ?? 1), tone },
						{ theme: icons.theme, revision: icons.revision },
					)
				: null,
		[usable, group, extensionOrGroup, special, size, tone, icons.theme, icons.revision],
	);
	const image = useSystemImage(url);
	useEffect(() => {
		if (url !== null && image === 'idle') requestSystemImage(url);
	}, [url, image]);
	if (url === null || image !== 'loaded') return <>{fallback}</>;
	return (
		<svg
			className={className ? `${styles.icon} ${className}` : styles.icon}
			data-group={group}
			data-system=""
			viewBox="0 0 16 16"
			aria-hidden="true"
			focusable="false"
		>
			<image href={url} width="16" height="16" preserveAspectRatio="xMidYMid meet" />
		</svg>
	);
}
