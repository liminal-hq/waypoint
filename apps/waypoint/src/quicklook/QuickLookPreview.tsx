// The body of Quick Look: an entry's content by kind, with a fallback panel for a preview that fails
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import type { TextHead } from '@liminal-hq/waypoint-protocol/generated/TextHead';
import { useEffect, useState } from 'react';
import { formatModified, formatSize } from '../browse/format';
import { t } from '../i18n/messages';
import type { DetailsClient } from '../services/detailsClient';
import type { HourCycle } from '../services/timeFormatClient';
import { Thumbnail } from '../thumbnails/Thumbnail';
import { entryThumbKey, wantsThumbnail } from '../thumbnails/thumbnailModel';
import type { ThumbnailLoader } from '../thumbnails/thumbnailLoader';
import { kindMessage, previewKindOf, type PreviewKind } from './quickLookModel';
import styles from './QuickLook.module.css';

interface QuickLookPreviewProps {
	entry: Entry;
	handle: ListingHandle;
	client: DetailsClient;
	loader: ThumbnailLoader<{ key: string }> | null;
	hourCycle: HourCycle | undefined;
}

/**
 * Shows `entry` by its kind. The thumbnail, where there is one, is drawn first and stays under
 * the real content, so a step shows something at once. Anything that cannot be shown falls back to
 * the facts panel with a sentence saying why; the stage is never left blank. The parent keys this
 * on the entry, so each entry starts from a clean state.
 */
export function QuickLookPreview(props: QuickLookPreviewProps) {
	const { entry, handle, client } = props;
	const kind = previewKindOf(entry);
	const [failure, setFailure] = useState<string | null>(null);
	const fail = (message: string) => setFailure(message);

	if (failure !== null) return <FactsPanel {...props} kind={kind} note={failure} />;
	switch (kind) {
		case 'image':
			return (
				<div className={styles.picture}>
					<Pictured {...props} className={styles.underlay} />
					<img
						className={styles.image}
						src={client.previewUrl(handle, entry.id)}
						alt={entry.name}
						draggable={false}
						onError={() => fail(t('quickLook.failed'))}
					/>
				</div>
			);
		case 'video':
			return (
				<video
					className={styles.video}
					src={client.previewUrl(handle, entry.id)}
					controls
					preload="metadata"
					aria-label={entry.name}
					onError={() => fail(t('quickLook.failed.media'))}
				/>
			);
		case 'audio':
			return (
				<div className={styles.audio}>
					<Pictured {...props} className={styles.large} />
					<audio
						className={styles.audioControls}
						src={client.previewUrl(handle, entry.id)}
						controls
						preload="metadata"
						aria-label={entry.name}
						onError={() => fail(t('quickLook.failed.media'))}
					/>
				</div>
			);
		case 'text':
			return <TextPreview {...props} onFail={fail} />;
		default:
			return <FactsPanel {...props} kind={kind} note={null} />;
	}
}

/** The entry's thumbnail over its icon, in a frame the caller sizes (the large frame is 160 px). */
function Pictured({
	entry,
	handle,
	loader,
	className,
}: Pick<QuickLookPreviewProps, 'entry' | 'handle' | 'loader'> & { className: string | undefined }) {
	return (
		<Thumbnail
			loader={loader}
			thumbKey={wantsThumbnail(entry) ? entryThumbKey(entry) : null}
			group={entry.group}
			name={entry.name}
			source={{ handle, id: entry.id, modifiedMs: entry.modifiedMs }}
			iconSize={160}
			{...(className ? { className } : {})}
		/>
	);
}

type TextState = { status: 'loading' } | { status: 'ready'; head: TextHead };

function TextPreview({
	entry,
	handle,
	client,
	onFail,
}: QuickLookPreviewProps & { onFail: (message: string) => void }) {
	const [state, setState] = useState<TextState>({ status: 'loading' });
	useEffect(() => {
		let live = true;
		client.readTextHead(handle, entry.id).then(
			(head) => {
				if (live) setState({ status: 'ready', head });
			},
			(error: unknown) => {
				if (!live) return;
				// A binary file is not an error: it is previewed as the kind of file it is.
				const binary = (error as { kind?: string } | null)?.kind === 'notText';
				onFail(binary ? t('quickLook.noPreview') : t('quickLook.failed.text'));
			},
		);
		return () => {
			live = false;
		};
		// `onFail` only sets state for the entry this was keyed on.
	}, [client, handle, entry.id]);

	if (state.status === 'loading') {
		return (
			<p className={styles.status} role="status">
				{t('quickLook.loading')}
			</p>
		);
	}
	const { head } = state;
	return (
		<div className={styles.text}>
			{head.text === '' ? (
				<p className={styles.status}>{t('quickLook.empty')}</p>
			) : (
				<pre className={styles.code} data-selectable="">
					{head.text}
				</pre>
			)}
			{head.truncated && (
				<p className={styles.notice} data-testid="quicklook-truncated">
					{t('quickLook.truncated')}
				</p>
			)}
			{head.lossy && <p className={styles.notice}>{t('quickLook.lossy')}</p>}
		</div>
	);
}

/** The large thumbnail (or icon) with the name, kind and size: PDFs, fonts, archives, folders and everything else. */
function FactsPanel({
	entry,
	handle,
	loader,
	hourCycle,
	kind,
	note,
}: QuickLookPreviewProps & { kind: PreviewKind; note: string | null }) {
	return (
		<div className={styles.facts}>
			<Pictured entry={entry} handle={handle} loader={loader} className={styles.large} />
			<p className={styles.name}>{entry.name}</p>
			<dl className={styles.list}>
				<dt>{t('quickLook.fact.kind')}</dt>
				<dd>{t(kindMessage(kind, entry))}</dd>
				{entry.size !== null && entry.kind !== 'directory' && (
					<>
						<dt>{t('quickLook.fact.size')}</dt>
						<dd>{formatSize(entry.size)}</dd>
					</>
				)}
				{entry.modifiedMs !== null && (
					<>
						<dt>{t('quickLook.fact.modified')}</dt>
						<dd>{formatModified(entry.modifiedMs, undefined, hourCycle)}</dd>
					</>
				)}
			</dl>
			{note !== null && (
				<p className={styles.status} role="status">
					{note}
				</p>
			)}
		</div>
	);
}
