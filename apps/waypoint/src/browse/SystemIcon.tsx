// The icon the operating system shows for an entry's type, over the Waypoint glyph until it has loaded and wherever the system has none
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useMemo, type ReactNode } from 'react';
import type { IconGroup } from '@liminal-hq/waypoint-protocol/generated/IconGroup';
import type { SpecialFolder } from '@liminal-hq/waypoint-protocol/generated/SpecialFolder';
import type { FolderTone } from '../icons/portage/portagePalette';
import {
	hasOwnIcon,
	iconScale,
	iconSizeFor,
	systemIconTarget,
	type EntryIconSource,
} from '../icons/systemIconTarget';
import {
	requestSystemImage,
	systemFileIconUrl,
	systemIconUrl,
	useSystemIcons,
	useSystemImage,
} from '../icons/systemIcons';
import { cssUrl } from '../icons/iconPictures';
import styles from './FileIcon.module.css';

interface SystemIconProps {
	group: IconGroup;
	special?: SpecialFolder | null;
	/** The entry's name, whose extension picks the type's icon. Without it the group's stand-in type is drawn. */
	name?: string;
	/** The entry the icon is for, so a program or a shortcut is drawn from its own icon; the type's icon is drawn where there is none. */
	source?: EntryIconSource;
	/** The size the icon is drawn at in CSS pixels, so the picture asked for is no bigger than it needs to be. */
	size: number;
	tone: FolderTone;
	className?: string;
	/** What is drawn until the system's icon is there, and for good where the system has none. Never nothing. */
	fallback: ReactNode;
	/** Draw the icon as a background picture (under `IconPictures`) rather than an SVG `<image>`. */
	picture?: boolean;
}

/**
 * The system's icon for the entry's type, as an SVG `<image>` in the same frame the Waypoint glyph has, so
 * every rule that sizes the glyph sizes this too. The icon is asked for by type (the extension, or the
 * group's stand-in type, or the kind of folder), so a listing of thousands of files asks for a handful of
 * pictures, and each address is loaded once and shared by every row with it. The few kinds of file that
 * carry their own icon (programs, shortcuts, icons and cursors) are asked for by file as well, and draw the
 * type's icon only when that has none. Decorative: the row's name is the accessible name.
 */
export function SystemIcon({
	group,
	special,
	name,
	source,
	size,
	tone,
	className,
	fallback,
	picture = false,
}: SystemIconProps) {
	const icons = useSystemIcons();
	const kind = group === 'folder' ? icons.folder : icons.type;
	const usable = icons.phase === 'ready' && kind.available;
	const extensionOrGroup = group === 'folder' ? null : name;
	const edge = iconSizeFor(size);
	const scale = iconScale(globalThis.devicePixelRatio ?? 1);
	const handle = source?.handle;
	const entry = source?.id;
	const modifiedMs = source?.modifiedMs ?? null;
	const own = usable && handle !== undefined && group !== 'folder' && hasOwnIcon(name);
	const fileUrl = useMemo(
		() =>
			own && handle !== undefined
				? systemFileIconUrl(`${handle}-${entry}`, { size: edge, scale, modifiedMs })
				: null,
		[own, handle, entry, modifiedMs, edge, scale],
	);
	const fileImage = useSystemImage(fileUrl);
	useEffect(() => {
		if (fileUrl !== null && fileImage === 'idle') requestSystemImage(fileUrl);
	}, [fileUrl, fileImage]);
	// The type's icon is wanted when the file has none of its own to ask for, or the system had none to give.
	const typeWanted = usable && (fileUrl === null || fileImage === 'missing');
	const typeUrl = useMemo(
		() =>
			typeWanted
				? systemIconUrl(
						systemIconTarget(group, extensionOrGroup ?? undefined, special),
						{ size: edge, scale, tone },
						{ theme: icons.theme, revision: icons.revision },
					)
				: null,
		[typeWanted, group, extensionOrGroup, special, edge, scale, tone, icons.theme, icons.revision],
	);
	const typeImage = useSystemImage(typeUrl);
	useEffect(() => {
		if (typeUrl !== null && typeImage === 'idle') requestSystemImage(typeUrl);
	}, [typeUrl, typeImage]);
	const fileLoaded = fileUrl !== null && fileImage === 'loaded';
	const url = fileLoaded ? fileUrl : typeUrl;
	if (url === null || (!fileLoaded && typeImage !== 'loaded')) return <>{fallback}</>;
	if (picture) {
		return (
			<span
				className={className ? `${styles.picture} ${className}` : styles.picture}
				data-group={group}
				data-system=""
				data-icon-picture=""
				aria-hidden="true"
				style={{ backgroundImage: cssUrl(url) }}
			/>
		);
	}
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
