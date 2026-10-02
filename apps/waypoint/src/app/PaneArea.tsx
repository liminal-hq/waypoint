// The panes of the active tab: one file view, or a pair's views with dividers and headers
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { Pair } from '@liminal-hq/waypoint-protocol/generated/Pair';
import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import {
	Fragment,
	useCallback,
	useEffect,
	useRef,
	useState,
	useSyncExternalStore,
	type PointerEvent,
} from 'react';
import { FileView } from '../browse/FileView';
import type { MenuRequest } from '../browse/useListInteractions';
import type { ListingSession, SessionState } from '../browse/useListingSession';
import type { ViewMode } from '../browse/viewStore';
import { dropAttributes } from '../dnd/dropTargets';
import { tf } from '../i18n/messages';
import { useNavigation } from '../nav/useNavigation';
import { useOpenEntry, type EntryAction, type EntryOpeners } from '../nav/useOpenEntry';
import { PaneDivider } from '../tabs/PaneDivider';
import { SplitZones } from '../tabs/SplitZones';
import { PaneHeader } from '../tabs/PaneHeader';
import { clearPaneFocus, subscribePaneFocus, wantedPaneFocus } from '../tabs/paneFocus';
import { equalSizes } from '../tabs/pairLayout';
import { usePairActions } from '../tabs/pairActions';
import { useSeparateDrag } from '../tabs/paneDrag';
import { useTabActions } from '../tabs/tabActions';
import { locationLabel, useTabTitle } from '../tabs/tabTitle';
import styles from './PaneArea.module.css';

/** A menu request from a pane, with what the menu needs to act on that pane and not on whichever is active. */
export type PaneMenuRequest = MenuRequest & {
	openers: EntryOpeners;
	session: ListingSession | null;
};

interface PaneAreaProps {
	/** The panes in order: one for a single tab, a pair's panes otherwise. */
	panes: readonly TabSnapshot[];
	/** Set when `panes` are a pair's. */
	pair: Pair | undefined;
	active: TabId | null;
	stateFor: (tab: TabId) => SessionState;
	mode: ViewMode;
	gridSize: number;
	onFailure: (entry: Entry, action: EntryAction) => void;
	onMenu: (request: PaneMenuRequest) => void;
}

interface LiveSizes {
	pair: number;
	sizes: number[];
}

/**
 * A single tab fills the area as it always has. A pair shows each pane under a header, separated
 * by dividers, side by side or stacked, each pane sized by its share of the pair. A drag resizes
 * in memory (`live`) and the sizes go to the session when it is released; the session's own value
 * takes over again as soon as it arrives.
 */
export function PaneArea({
	panes,
	pair,
	active,
	stateFor,
	mode,
	gridSize,
	onFailure,
	onMenu,
}: PaneAreaProps) {
	const pairs = usePairActions();
	const tabActions = useTabActions();
	const grip = useSeparateDrag(pair?.id);
	const [live, setLive] = useState<LiveSizes | null>(null);
	const area = useRef<HTMLDivElement | null>(null);
	const paired = pair !== undefined && panes.length > 1;

	const keptKey = pair ? `${pair.id}:${pair.sizes.join(',')}` : '';
	useEffect(() => setLive(null), [keptKey]);

	const sizes: readonly number[] =
		paired && live?.pair === pair.id && live.sizes.length === panes.length
			? live.sizes
			: (pair?.sizes ?? equalSizes(panes.length));

	const commit = useCallback(
		(next: number[]) => {
			if (!pair) return;
			setLive({ pair: pair.id, sizes: next });
			pairs.setSizes(pair, next).catch((error: unknown) => {
				console.warn('could not save the pane sizes', error);
				setLive(null);
			});
		},
		[pair, pairs],
	);

	// F3 and F6 move focus with the active pane once its list is on screen.
	const wanted = useSyncExternalStore(subscribePaneFocus, wantedPaneFocus);
	useEffect(() => {
		if (wanted === null) return;
		// The session makes the pane active a moment after the request: keep waiting until it is
		// (the request expires on its own if it never is).
		if (wanted !== active || !panes.some((pane) => pane.id === wanted)) return;
		// Wait for the listing: its list is what takes focus; a pane that failed to open takes it itself.
		if (stateFor(wanted).status === 'opening') return;
		const pane = area.current?.querySelector<HTMLElement>(`[data-pane="${wanted}"]`);
		const target = pane?.querySelector<HTMLElement>('[role="listbox"]') ?? pane;
		if (target) {
			target.focus();
			clearPaneFocus();
		}
	});

	return (
		<div
			ref={area}
			className={styles.area}
			data-layout={paired ? pair.layout : 'single'}
			// A tab drag measures this once, when it begins, for the split regions.
			data-pane-area=""
		>
			{panes.map((tab, index) => (
				<Fragment key={tab.id}>
					{paired && index > 0 ? (
						<PaneDivider
							layout={pair.layout}
							index={index - 1}
							sizes={sizes}
							onResize={(next) => setLive({ pair: pair.id, sizes: next })}
							onCommit={commit}
							onCancel={() => setLive(null)}
							onReset={() => commit(equalSizes(panes.length))}
						/>
					) : null}
					<Pane
						tab={tab}
						index={index}
						count={panes.length}
						paired={paired}
						share={paired ? (sizes[index] ?? 0) : 1}
						active={tab.id === active}
						state={stateFor(tab.id)}
						mode={mode}
						gridSize={gridSize}
						onFailure={onFailure}
						onMenu={onMenu}
						onActivate={tabActions.activate}
						onClose={pairs.closePane}
						onGrip={(event) => grip(event, tab.id)}
					/>
				</Fragment>
			))}
			<SplitZones />
		</div>
	);
}

interface PaneProps {
	tab: TabSnapshot;
	index: number;
	count: number;
	paired: boolean;
	share: number;
	active: boolean;
	state: SessionState;
	mode: ViewMode;
	gridSize: number;
	onFailure: (entry: Entry, action: EntryAction) => void;
	onMenu: (request: PaneMenuRequest) => void;
	onActivate: (tab: TabId) => void;
	onClose: (tab: TabId) => void;
	onGrip: (event: PointerEvent<HTMLElement>) => void;
}

function Pane({
	tab,
	index,
	count,
	paired,
	share,
	active,
	state,
	mode,
	gridSize,
	onFailure,
	onMenu,
	onActivate,
	onClose,
	onGrip,
}: PaneProps) {
	const title = useTabTitle(tab);
	// This pane's own navigation, so opening a folder here moves this pane's tab even for the
	// moment before the session has made the pane active.
	const navigation = useNavigation(tab.id);
	const openers = useOpenEntry(navigation, onFailure);
	const session = state.status === 'ready' ? state.session : null;
	const activate = () => {
		if (!active) onActivate(tab.id);
	};
	return (
		<div
			className={styles.pane}
			data-pane={tab.id}
			{...dropAttributes('pane', tab.id, locationLabel(tab.location))}
			tabIndex={-1}
			data-active={paired && active ? '' : undefined}
			style={{ flexGrow: share }}
			role={paired ? 'group' : undefined}
			aria-label={paired ? tf('pair.pane.label', { position: index + 1, count, title }) : undefined}
			// A press or focus anywhere in the pane makes it the active one (the file list, the header).
			onPointerDownCapture={activate}
			onFocusCapture={activate}
		>
			{paired ? (
				<PaneHeader tab={tab} active={active} onClose={() => onClose(tab.id)} onGrip={onGrip} />
			) : null}
			<div className={styles.content}>
				<FileView
					state={state}
					mode={mode}
					gridSize={gridSize}
					location={tab.location}
					onOpen={openers.open}
					onOpenInNewTab={openers.openInNewTab}
					onMenu={(request) => onMenu({ ...request, openers, session })}
				/>
			</div>
		</div>
	);
}
