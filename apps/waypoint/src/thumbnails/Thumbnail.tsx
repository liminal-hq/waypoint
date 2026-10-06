// An entry's picture over its icon: the icon until the thumbnail loads, and again if it cannot
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { IconGroup } from '@liminal-hq/waypoint-protocol/generated/IconGroup';
import type { SpecialFolder } from '@liminal-hq/waypoint-protocol/generated/SpecialFolder';
import { useCallback, useContext, useState, useSyncExternalStore } from 'react';
import { FileIcon } from '../browse/FileIcon';
import type { PortageFolderBadge } from '../icons/portage/portageFolderArt';
import styles from './Thumbnail.module.css';
import type { ThumbnailLoader } from './thumbnailLoader';
import { ThumbnailPauseContext } from './thumbnailPause';

interface ThumbnailProps {
	/** `null` where no thumbnails are asked for (off, unavailable, or a view too small for them). */
	loader: ThumbnailLoader<{ key: string }> | null;
	/** The loader's key for this entry; `null` for one with no thumbnail to ask for (a folder). */
	thumbKey: string | null;
	group: IconGroup;
	/** The entry's name, for the System icon set: its extension picks the type's icon. */
	name?: string;
	/** The size the icon is drawn at in CSS pixels, when it is not the list's 16. */
	iconSize?: number;
	/** The standard folder a folder is, so the icon carries its mark. */
	special?: SpecialFolder | null;
	/** A sticker the Portage set draws on a folder, such as the Git mark of a repository. */
	badge?: PortageFolderBadge | undefined;
	/** Sizes the frame; the icon and the picture fill it. */
	className?: string;
	/** The class the icon alone is drawn with. */
	iconClassName?: string;
}

const noUnsubscribe = () => undefined;
const noSubscribe = () => noUnsubscribe;

/**
 * The icon in a frame of the caller's size, with the thumbnail drawn over it once it is ready. The
 * picture is decorative (`alt` is empty): the entry's name is its accessible name. A picture that
 * cannot be loaded is dropped and the icon stays, so nothing is ever hidden for lack of one.
 */
export function Thumbnail({
	loader,
	thumbKey,
	group,
	name,
	iconSize,
	special,
	badge,
	className,
	iconClassName,
}: ThumbnailProps) {
	const subscribe = useCallback(
		(listener: () => void) =>
			loader && thumbKey !== null ? loader.subscribe(thumbKey, listener) : noUnsubscribe,
		[loader, thumbKey],
	);
	const url = useSyncExternalStore(subscribe, () =>
		loader && thumbKey !== null ? loader.urlOf(thumbKey) : null,
	);
	const [shown, setShown] = useState<{
		url: string;
		state: 'loading' | 'loaded' | 'broken';
	} | null>(null);
	const state = url !== null && shown?.url === url ? shown.state : url !== null ? 'loading' : null;
	// While the view scrolls, a picture not drawn here yet waits (the icon stays) until it stops. Only
	// a frame with a picture to wait for reads the pause, so a scroll starting or stopping redraws
	// those few and not every frame in view.
	const pause = useContext(ThumbnailPauseContext);
	const waiting = url !== null && state !== 'loaded';
	const held = useSyncExternalStore(
		pause && waiting ? pause.subscribe : noSubscribe,
		() => waiting && pause !== null && pause.paused(),
	);
	return (
		<span
			className={className ? `${styles.frame} ${className}` : styles.frame}
			data-thumbnail={state ?? undefined}
		>
			<FileIcon
				group={group}
				special={special}
				name={name}
				size={iconSize}
				badge={badge}
				className={iconClassName}
			/>
			{url !== null && state !== 'broken' && !held && (
				<img
					// A new element for each picture: a recycled grid cell must not show the last item's
					// picture (an image keeps its old picture until the new one loads) or fade it out.
					key={url}
					className={styles.picture}
					src={url}
					alt=""
					// Decoded off the main thread, so a screen of pictures arriving does not hold up a frame.
					decoding="async"
					draggable={false}
					onLoad={() => setShown({ url, state: 'loaded' })}
					onError={() => setShown({ url, state: 'broken' })}
				/>
			)}
		</span>
	);
}
