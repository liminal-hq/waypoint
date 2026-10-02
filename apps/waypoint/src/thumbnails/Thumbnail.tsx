// An entry's picture over its icon: the icon until the thumbnail loads, and again if it cannot
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { IconGroup } from '@liminal-hq/waypoint-protocol/generated/IconGroup';
import { useCallback, useState, useSyncExternalStore } from 'react';
import { FileIcon } from '../browse/FileIcon';
import styles from './Thumbnail.module.css';
import type { ThumbnailLoader } from './thumbnailLoader';

interface ThumbnailProps {
	/** `null` where no thumbnails are asked for (off, unavailable, or a view too small for them). */
	loader: ThumbnailLoader<{ key: string }> | null;
	/** The loader's key for this entry; `null` for one with no thumbnail to ask for (a folder). */
	thumbKey: string | null;
	group: IconGroup;
	/** Sizes the frame; the icon and the picture fill it. */
	className?: string;
	/** The class the icon alone is drawn with. */
	iconClassName?: string;
}

const noUnsubscribe = () => undefined;

/**
 * The icon in a frame of the caller's size, with the thumbnail drawn over it once it is ready. The
 * picture is decorative (`alt` is empty): the entry's name is its accessible name. A picture that
 * cannot be loaded is dropped and the icon stays, so nothing is ever hidden for lack of one.
 */
export function Thumbnail({ loader, thumbKey, group, className, iconClassName }: ThumbnailProps) {
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
	return (
		<span
			className={className ? `${styles.frame} ${className}` : styles.frame}
			data-thumbnail={state ?? undefined}
		>
			<FileIcon group={group} className={iconClassName} />
			{url !== null && state !== 'broken' && (
				<img
					className={styles.picture}
					src={url}
					alt=""
					draggable={false}
					onLoad={() => setShown({ url, state: 'loaded' })}
					onError={() => setShown({ url, state: 'broken' })}
				/>
			)}
		</span>
	);
}
