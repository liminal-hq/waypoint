// The Inspector: a panel at the right edge of the body with Preview and Properties tabs
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import {
	useEffect,
	useId,
	useRef,
	useState,
	type KeyboardEvent,
	type PointerEvent,
	type ReactNode,
} from 'react';
import { useStore } from 'zustand';
import type { ListingSession } from '../browse/useListingSession';
import { t, tf, tn } from '../i18n/messages';
import { CloseSmallIcon } from '../icons/AppIcons';
import { useDetailsClient } from './DetailsClientContext';
import { EntryPreview } from './EntryPreview';
import {
	DEFAULT_WIDTH,
	MAX_WIDTH,
	MIN_WIDTH,
	useInspectorStore,
	type InspectorTab,
} from './inspectorStore';
import { baseName } from './inspectorModel';
import { PropertiesPane } from './PropertiesPane';
import { useEntryDetails } from './useEntryDetails';
import { useInspectorSubject, type InspectorSubject } from './useInspectorSubject';
import { FileIcon } from '../browse/FileIcon';
import styles from './InspectorPanel.module.css';

/** How far an arrow key moves the divider, and how far with Shift. */
const DIVIDER_STEP = 16;
const DIVIDER_BIG_STEP = 64;

const TABS: ReadonlyArray<InspectorTab> = ['preview', 'properties'];

/**
 * The docked Inspector, shown while the window's store says it is open. It follows the active
 * pane's selection (`useInspectorSubject`) and reads nothing until it is on screen, so a closed
 * Inspector costs nothing. The tabs are a roving-tabindex tablist (Left and Right, Home and End
 * move and select), each with a labelled tabpanel; the left edge is a separator that resizes it.
 */
export function InspectorPanel({
	session,
	location,
}: {
	/** The active pane's listing. */
	session: ListingSession | null;
	/** The folder that listing shows. */
	location: Location | undefined;
}) {
	const store = useInspectorStore();
	const tab = useStore(store, (s) => s.tab);
	const width = useStore(store, (s) => s.width);
	const idBase = useId();
	const tabRefs = useRef(new Map<InspectorTab, HTMLButtonElement>());
	const client = useDetailsClient();
	const subject = useInspectorSubject(session, location);
	const entry = subject.kind === 'entry' ? subject.entry : null;
	const handle = subject.kind === 'entry' ? subject.handle : null;
	const details = useEntryDetails(client, handle, entry);

	const tabId = (which: InspectorTab) => `${idBase}-tab-${which}`;
	const panelId = (which: InspectorTab) => `${idBase}-panel-${which}`;

	const onTabKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
		const rtl = getComputedStyle(event.currentTarget).direction === 'rtl';
		const at = TABS.indexOf(tab);
		let next: number | null = null;
		if (event.key === (rtl ? 'ArrowLeft' : 'ArrowRight')) next = (at + 1) % TABS.length;
		else if (event.key === (rtl ? 'ArrowRight' : 'ArrowLeft')) {
			next = (at - 1 + TABS.length) % TABS.length;
		} else if (event.key === 'Home') next = 0;
		else if (event.key === 'End') next = TABS.length - 1;
		if (next === null) return;
		event.preventDefault();
		const to = TABS[next]!;
		store.getState().setTab(to);
		tabRefs.current.get(to)?.focus();
	};

	return (
		<aside
			className={styles.inspector}
			aria-label={t('inspector.label')}
			style={{ inlineSize: width }}
			data-inspector=""
		>
			<InspectorDivider />
			<div className={styles.header}>
				<div
					role="tablist"
					aria-label={t('inspector.tabs.label')}
					className={styles.tabs}
					onKeyDown={onTabKeyDown}
				>
					{TABS.map((which) => (
						<button
							key={which}
							ref={(element) => {
								if (element) tabRefs.current.set(which, element);
								else tabRefs.current.delete(which);
							}}
							type="button"
							role="tab"
							id={tabId(which)}
							className={styles.tab}
							aria-selected={tab === which}
							aria-controls={panelId(which)}
							tabIndex={tab === which ? 0 : -1}
							onClick={() => store.getState().setTab(which)}
						>
							{t(which === 'preview' ? 'inspector.tab.preview' : 'inspector.tab.properties')}
						</button>
					))}
				</div>
				<button
					type="button"
					className={styles.close}
					aria-label={t('inspector.close')}
					title={t('inspector.close')}
					onClick={() => store.getState().setOpen(false)}
				>
					<CloseSmallIcon />
				</button>
			</div>
			{TABS.map((which) => (
				<div
					key={which}
					role="tabpanel"
					id={panelId(which)}
					aria-labelledby={tabId(which)}
					className={styles.panel}
					hidden={tab !== which}
				>
					{tab === which &&
						(which === 'preview' ? (
							<PreviewBody subject={subject} client={client} details={details} />
						) : (
							<PropertiesPane
								subject={subject}
								session={session}
								client={client}
								details={details}
								folder={location}
								active
							/>
						))}
				</div>
			))}
		</aside>
	);
}

/** The Preview tab for each kind of subject. */
function PreviewBody({
	subject,
	client,
	details,
}: {
	subject: InspectorSubject;
	client: ReturnType<typeof useDetailsClient>;
	details: ReturnType<typeof useEntryDetails>;
}): ReactNode {
	switch (subject.kind) {
		case 'none':
			return <p className={styles.empty}>{t('inspector.preview.empty')}</p>;
		case 'entry':
			return (
				<EntryPreview
					client={client}
					handle={subject.handle}
					entry={subject.entry}
					details={details.status === 'ready' ? details.details : null}
				/>
			);
		case 'many':
			return (
				<div className={styles.summary}>
					<p className={styles.summaryTitle}>{tn('browse.selection', subject.count)}</p>
				</div>
			);
		case 'folder':
			return (
				<div className={styles.summary}>
					<FileIcon group="folder" className={styles.bigIcon} />
					<p className={styles.summaryTitle} data-selectable="">
						{baseName(subject.location.display)}
					</p>
					<p className={styles.summaryFacts}>{tn('status.items', subject.count)}</p>
				</div>
			);
	}
}

/** The panel's left edge: drag it, or use the arrow keys, to resize; double-click restores the default. */
function InspectorDivider() {
	const store = useInspectorStore();
	const width = useStore(store, (s) => s.width);
	const drag = useRef<{
		startX: number;
		startWidth: number;
		sign: 1 | -1;
		pointerId: number;
	} | null>(null);
	const [dragging, setDragging] = useState(false);

	const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
		if (event.button !== 0) return;
		const rtl = getComputedStyle(event.currentTarget).direction === 'rtl';
		event.currentTarget.setPointerCapture?.(event.pointerId);
		event.preventDefault();
		event.currentTarget.focus();
		// The panel is docked at the end edge: dragging toward the start widens it.
		drag.current = {
			startX: event.clientX,
			startWidth: width,
			sign: rtl ? 1 : -1,
			pointerId: event.pointerId,
		};
		setDragging(true);
	};
	const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
		const state = drag.current;
		if (!state) return;
		store.getState().setWidth(state.startWidth + (event.clientX - state.startX) * state.sign);
	};
	const finish = (event: PointerEvent<HTMLDivElement>, keep: boolean) => {
		const state = drag.current;
		if (!state) return;
		drag.current = null;
		setDragging(false);
		event.currentTarget.releasePointerCapture?.(state.pointerId);
		if (!keep) store.getState().setWidth(state.startWidth);
	};
	const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
		const rtl = getComputedStyle(event.currentTarget).direction === 'rtl';
		const step = event.shiftKey ? DIVIDER_BIG_STEP : DIVIDER_STEP;
		const wider = rtl ? 'ArrowRight' : 'ArrowLeft';
		const narrower = rtl ? 'ArrowLeft' : 'ArrowRight';
		if (event.key === wider) store.getState().setWidth(width + step);
		else if (event.key === narrower) store.getState().setWidth(width - step);
		else return;
		event.preventDefault();
	};
	useEffect(() => {
		const onKey = (event: globalThis.KeyboardEvent) => {
			if (event.key !== 'Escape' || !drag.current) return;
			event.stopPropagation();
			store.getState().setWidth(drag.current.startWidth);
			drag.current = null;
			setDragging(false);
		};
		window.addEventListener('keydown', onKey, true);
		return () => window.removeEventListener('keydown', onKey, true);
	}, [store]);

	return (
		<div
			role="separator"
			tabIndex={0}
			className={styles.divider}
			aria-orientation="vertical"
			aria-label={t('inspector.divider.label')}
			aria-valuenow={width}
			aria-valuemin={MIN_WIDTH}
			aria-valuemax={MAX_WIDTH}
			aria-valuetext={tf('inspector.divider.value', { width })}
			data-dragging={dragging ? '' : undefined}
			onPointerDown={onPointerDown}
			onPointerMove={onPointerMove}
			onPointerUp={(event) => finish(event, true)}
			onPointerCancel={(event) => finish(event, false)}
			onDoubleClick={() => store.getState().setWidth(DEFAULT_WIDTH)}
			onKeyDown={onKeyDown}
		/>
	);
}

/** The dock: the panel while the Inspector is open, nothing otherwise. */
export function InspectorDock({
	session,
	location,
}: {
	session: ListingSession | null;
	location: Location | undefined;
}) {
	const store = useInspectorStore();
	const open = useStore(store, (s) => s.open);
	return open ? <InspectorPanel session={session} location={location} /> : null;
}
