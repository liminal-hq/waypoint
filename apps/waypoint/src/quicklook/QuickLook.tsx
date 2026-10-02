// The Quick Look overlay: a modal preview of the focused entry that the arrow keys step through
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Dialog } from '@liminal-hq/waypoint-chrome/Dialog/Dialog';
import { DialogActions, DialogButton } from '@liminal-hq/waypoint-chrome/Dialog/DialogActions';
import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import {
	useCallback,
	useEffect,
	useRef,
	useSyncExternalStore,
	type KeyboardEvent as ReactKeyboardEvent,
} from 'react';
import { useStore } from 'zustand';
import { useHourCycle } from '../browse/TimeFormatContext';
import { useVfsClient } from '../browse/VfsClientContext';
import type { OpenHandler } from '../browse/useListInteractions';
import type { ListingSession } from '../browse/useListingSession';
import { t, tf } from '../i18n/messages';
import { openWithChooserStore } from '../openWith/openWithChooserStore';
import { runOpenWithCommand, openWithCommandAvailable } from '../openWith/openWithCommand';
import { openWithAbilities, useOpenWithService } from '../openWith/OpenWithContext';
import type { DetailsClient } from '../services/detailsClient';
import { useEntryThumbnailLoader } from '../thumbnails/ThumbnailsContext';
import { entryThumbKey, wantsThumbnail } from '../thumbnails/thumbnailModel';
import { QuickLookPreview } from './QuickLookPreview';
import { isPreviewable, neighbourPositions, stepTarget } from './quickLookModel';
import type { ViewMove } from './quickLookStore';
import styles from './QuickLook.module.css';

interface QuickLookProps {
	session: ListingSession;
	move: ViewMove;
	client: DetailsClient;
	onOpen: OpenHandler | undefined;
	onClose: () => void;
}

/** The entry at `position`, read from the listing's cache and fetched when it is not there yet. */
function useEntryAt(session: ListingSession, position: number | null): Entry | undefined {
	const { model } = session;
	useSyncExternalStore(model.subscribe, model.getVersion);
	const entry = position === null ? undefined : model.entryAt(position);
	const missing = position !== null && !model.hasFresh(position);
	useEffect(() => {
		if (position !== null && missing) void model.readRange(position, position + 1);
	}, [model, position, missing]);
	return entry;
}

const isInteractive = (target: EventTarget) =>
	target instanceof HTMLElement &&
	target.closest('button, input, textarea, select, audio, video, a') !== null;

/**
 * Previews the focused entry of `session`. Stepping moves the listing's own focus and selection,
 * so the list is already on the right entry when the overlay closes, and the dialog gives the
 * focus trap, Esc and the return of focus to the list.
 */
export function QuickLook({ session, move, client, onOpen, onClose }: QuickLookProps) {
	const { model, store } = session;
	const focus = useStore(store, (state) => state.focus);
	const entry = useEntryAt(session, focus);
	const count = model.count;
	const hourCycle = useHourCycle();
	const stage = useRef<HTMLDivElement>(null);
	const stepping = useRef(false);

	// Neighbours' thumbnails only, so a step is cached; the content itself is fetched on arrival.
	const loader = useEntryThumbnailLoader(model.handle, 'x-large');
	useEffect(() => {
		if (!loader || focus === null) return;
		const ask = (position: number) => {
			const near = model.entryAt(position);
			return near && wantsThumbnail(near) ? { key: entryThumbKey(near), id: near.id } : null;
		};
		const items = [focus, ...neighbourPositions(focus, count)]
			.map(ask)
			.filter((item): item is NonNullable<typeof item> => item !== null);
		loader.want(items.slice(0, 1), items.slice(1));
	}, [loader, model, focus, count, entry]);

	// A listing that no longer has the focused position (a folder emptied behind the overlay) has nothing to preview.
	useEffect(() => {
		if (focus === null || focus >= count) onClose();
	}, [focus, count, onClose]);

	const step = useCallback(
		async (key: string) => {
			const from = store.getState().focus;
			const last = model.count - 1;
			if (from === null || stepping.current) return;
			const first = stepTarget(key, from, last, move);
			if (first === null) return;
			const direction = first > from ? 1 : -1;
			stepping.current = true;
			try {
				for (let at = first; at >= 0 && at <= last; at += direction) {
					const candidate = model.hasFresh(at)
						? model.entryAt(at)
						: (await model.readRange(at, at + 1))[0];
					if (!candidate || !isPreviewable(candidate)) continue;
					store.getState().moveTo(at, true);
					store.getState().requestScroll(at);
					return;
				}
			} finally {
				stepping.current = false;
			}
		},
		[model, move, store],
	);

	const onKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
		if (event.nativeEvent.isComposing || event.ctrlKey || event.metaKey || event.altKey) return;
		if (event.key === ' ') {
			// A control keeps its own Space (a button presses, the media controls play and pause).
			if (isInteractive(event.target)) return;
			event.preventDefault();
			if (!event.repeat) onClose();
			return;
		}
		if (event.key.startsWith('Arrow') && !isInteractive(event.target)) {
			event.preventDefault();
			void step(event.key);
		}
	};

	const openWith = useOpenWithService();
	const vfs = useVfsClient();
	const canOpenWith =
		openWith !== null && openWithCommandAvailable(openWithAbilities(openWith.status));

	const number = (value: number) => new Intl.NumberFormat().format(value);
	const position = focus === null ? '' : number(focus + 1);
	const total = number(count);
	const name = entry?.name ?? t('quickLook.loading');
	const announcement = entry ? tf('quickLook.positionAnnouncement', { name, position, total }) : '';

	return (
		<Dialog
			open
			size="large"
			title={name}
			description={entry ? tf('quickLook.position', { position, total }) : undefined}
			initialFocus={stage}
			onClose={onClose}
			footer={
				<DialogActions>
					<DialogButton onClick={() => void step('ArrowLeft')} disabled={!focus}>
						{t('quickLook.previous')}
					</DialogButton>
					<DialogButton onClick={() => void step('ArrowRight')} disabled={focus === count - 1}>
						{t('quickLook.next')}
					</DialogButton>
					{canOpenWith && entry && (
						<DialogButton
							onClick={() => {
								onClose();
								store.getState().moveTo(focus ?? 0, true);
								void runOpenWithCommand(session, {
									client: openWith.client,
									status: openWith.status,
									vfs,
									chooser: openWithChooserStore,
								});
							}}
						>
							{t('quickLook.openWith')}
						</DialogButton>
					)}
					<DialogButton
						variant="primary"
						disabled={!entry}
						onClick={() => {
							onClose();
							if (entry) onOpen?.(entry, model.handle);
						}}
					>
						{t('quickLook.open')}
					</DialogButton>
					<DialogButton onClick={onClose}>{t('quickLook.close')}</DialogButton>
				</DialogActions>
			}
		>
			<div ref={stage} className={styles.stage} tabIndex={-1} onKeyDown={onKeyDown}>
				{entry ? (
					<QuickLookPreview
						key={`${model.handle}:${entry.id}:${entry.modifiedMs ?? 0}`}
						entry={entry}
						handle={model.handle}
						client={client}
						loader={loader}
						hourCycle={hourCycle}
					/>
				) : (
					<p className={styles.status} role="status">
						{t('quickLook.loading')}
					</p>
				)}
				<p className={styles.visuallyHidden} role="status" aria-live="polite">
					{announcement}
				</p>
			</div>
		</Dialog>
	);
}
