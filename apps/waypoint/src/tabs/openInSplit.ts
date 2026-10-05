// Opens a folder in a new pane beside the pane on show: the drop on a split zone, and its menu route
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import { t, tf } from '../i18n/messages';
import type { TabsApi } from '../services/tabsApi';
import type { Edge } from './dragLayout';
import { requestPaneFocus } from './paneFocus';
import { edgeLayout } from './tabDrag';
import { locationLabel } from './tabTitle';

export interface OpenInSplitDeps {
	api: TabsApi;
	/** The newest session snapshot, read when the split is made. */
	snapshot(): SessionSnapshot | null;
	/** The live region's feed. */
	announce(text: string): void;
}

/**
 * Whether a split can be made now: there is a tab on show, and neither it nor anything it could
 * join is already in a pair (a pair of three or more panes is not offered). This is what the
 * drag's split zones and the menu item both ask.
 */
export function canOpenInSplit(snapshot: SessionSnapshot | null): boolean {
	if (!snapshot || snapshot.active === null) return false;
	const active = snapshot.active;
	return !snapshot.pairs.some((pair) => pair.panes.includes(active));
}

/**
 * Opens `location` in a new tab joined to the tab on show as a pair, on `edge` of it (the right by
 * default, as the menu's Open in Split Pane does). The pane on show leads and keeps its place and
 * group; the new pane comes to the front and takes focus. Says what happened, or why nothing did.
 */
export async function openInSplit(
	deps: OpenInSplitDeps,
	location: Location,
	edge: Edge = 'right',
): Promise<boolean> {
	const snapshot = deps.snapshot();
	const title = locationLabel(location);
	if (!snapshot || snapshot.active === null) return false;
	if (!canOpenInSplit(snapshot)) {
		deps.announce(t('pair.announce.splitUnavailable'));
		return false;
	}
	const { api } = deps;
	const active = snapshot.active;
	const { layout, before } = edgeLayout(edge);
	try {
		const created = await api.openTab(location, { after: active, activate: false });
		const pair = await api.joinPair([active, created], layout);
		if (before) await api.swapPanes(pair);
		await api.activateTab(created);
		requestPaneFocus(created);
		deps.announce(tf('pair.announce.openedSplit', { title }));
		return true;
	} catch (error) {
		console.warn('could not open the folder in a split pane', error);
		deps.announce(tf('pair.announce.splitFailed', { title }));
		return false;
	}
}
