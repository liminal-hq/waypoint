// A preview of one entry: its picture, text, sound or video, read only as far as it is worth
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { EntryDetails } from '@liminal-hq/waypoint-protocol/generated/EntryDetails';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import { useEffect, useState } from 'react';
import { tf, t } from '../i18n/messages';
import type { DetailsClient } from '../services/detailsClient';
import { Thumbnail } from '../thumbnails/Thumbnail';
import { useEntryThumbnailLoader } from '../thumbnails/ThumbnailsContext';
import { entryThumbKey, wantsThumbnail } from '../thumbnails/thumbnailModel';
import { formatSize } from '../browse/format';
import {
	HEAVY_DELAY_MS,
	IMAGE_MAX_BYTES,
	TEXT_HEAD_BYTES,
	isFolderKind,
	kindMessage,
	previewKind,
} from './inspectorModel';
import { useSettled } from './useSettled';
import styles from './EntryPreview.module.css';

interface EntryPreviewProps {
	/** The service that reads files; without it the preview is the picture the views already have, or the icon. */
	client: DetailsClient | null;
	handle: ListingHandle;
	entry: Entry;
	/** What is known of the entry so far; the preview refines itself when it arrives. */
	details: EntryDetails | null;
}

type TextState =
	| { key: string; status: 'ready'; text: string; truncated: boolean; lossy: boolean }
	| { key: string; status: 'none' };

/**
 * The preview body, shared by the Inspector and Quick Look. It shows the entry's large thumbnail
 * (or its icon) at once, which is all that a selection moving quickly ever loads. Once the
 * selection has held still it upgrades by kind: an image on its `wpfile` address (a thumbnail
 * stays for one over `IMAGE_MAX_BYTES` or that the webview cannot draw), the first
 * `TEXT_HEAD_BYTES` of a text file in a monospace block, and an audio or video element that
 * fetches only its metadata until played. Anything else keeps the picture or icon.
 */
export function EntryPreview({ client, handle, entry, details }: EntryPreviewProps) {
	const key = `${handle}:${entry.id}:${entry.modifiedMs ?? 0}`;
	const settled = useSettled(key, HEAVY_DELAY_MS);
	const folder = isFolderKind(entry.kind, details?.resolvesTo ?? entry.linkTarget);
	const kind =
		folder || entry.kind === 'other' ? 'none' : previewKind(entry.group, details?.mimeType ?? null);
	const url = client && settled ? client.previewUrl(handle, entry.id) : null;

	// The picture the views use stands in while anything else loads, and is all a large image gets.
	const loader = useEntryThumbnailLoader(handle, 'large');
	const wanted = wantsThumbnail(entry);
	const thumbKey = wanted ? entryThumbKey(entry) : null;
	useEffect(() => {
		if (loader && thumbKey !== null) loader.want([{ key: thumbKey, id: entry.id }]);
	}, [loader, thumbKey, entry.id]);

	const [broken, setBroken] = useState<string | null>(null);
	const [loaded, setLoaded] = useState<string | null>(null);
	const size = details?.size ?? entry.size ?? 0;
	const showImage = kind === 'image' && url !== null && size <= IMAGE_MAX_BYTES && broken !== url;

	const [text, setText] = useState<TextState | null>(null);
	const wantsText = kind === 'text' && client !== null && settled;
	useEffect(() => {
		if (!wantsText || !client) return;
		let live = true;
		client.readTextHead(handle, entry.id, TEXT_HEAD_BYTES).then(
			(head) => {
				if (live) {
					setText({
						key,
						status: 'ready',
						text: head.text,
						truncated: head.truncated,
						lossy: head.lossy,
					});
				}
			},
			// A binary file, or one that went away: the icon stays.
			() => {
				if (live) setText({ key, status: 'none' });
			},
		);
		return () => {
			live = false;
		};
	}, [wantsText, client, handle, entry.id, key]);
	const head = text?.key === key ? text : null;

	const label = t(kindMessage(entry.kind, entry.group));
	return (
		<div className={styles.preview}>
			<div
				className={styles.stage}
				data-preview={
					showImage ? 'image' : head?.status === 'ready' ? 'text' : kind === 'none' ? 'icon' : kind
				}
			>
				<Thumbnail
					loader={loader}
					thumbKey={thumbKey}
					group={entry.group}
					name={entry.name}
					source={{ handle, id: entry.id, modifiedMs: entry.modifiedMs }}
					iconSize={64}
					className={styles.thumbnail}
				/>
				{showImage && (
					<img
						className={styles.image}
						src={url}
						alt={tf('inspector.preview.imageAlt', { name: entry.name })}
						data-loaded={loaded === url ? '' : undefined}
						draggable={false}
						onLoad={() => setLoaded(url)}
						onError={() => setBroken(url)}
					/>
				)}
				{kind === 'audio' && url !== null && (
					<audio
						className={styles.media}
						controls
						preload="metadata"
						src={url}
						aria-label={tf('inspector.preview.audioLabel', { name: entry.name })}
					/>
				)}
				{kind === 'video' && url !== null && (
					<video
						className={styles.media}
						controls
						preload="metadata"
						src={url}
						aria-label={tf('inspector.preview.videoLabel', { name: entry.name })}
					/>
				)}
			</div>
			<div className={styles.summary}>
				<p className={styles.name} data-selectable="">
					{entry.name}
				</p>
				<p className={styles.facts}>
					{label}
					{!folder && (details?.size ?? entry.size) !== null && (
						<> · {formatSize(details?.size ?? entry.size ?? 0)}</>
					)}
				</p>
			</div>
			<dl className={styles.rows}>
				<div className={styles.fact}>
					<dt>{t('inspector.field.kind')}</dt>
					<dd>{label}</dd>
				</div>
				{!folder && (details?.size ?? entry.size) !== null && (
					<div className={styles.fact}>
						<dt>{t('inspector.field.size')}</dt>
						<dd>{formatSize(details?.size ?? entry.size ?? 0)}</dd>
					</div>
				)}
			</dl>
			{head?.status === 'ready' && (
				<div className={styles.textBlock}>
					<pre
						className={styles.text}
						tabIndex={0}
						aria-label={tf('inspector.preview.textLabel', { name: entry.name })}
						data-selectable=""
					>
						{head.text}
					</pre>
					{head.truncated && (
						<p className={styles.notice}>
							{tf('inspector.preview.truncated', { size: formatSize(TEXT_HEAD_BYTES) })}
						</p>
					)}
					{head.lossy && <p className={styles.notice}>{t('inspector.preview.lossy')}</p>}
				</div>
			)}
		</div>
	);
}
