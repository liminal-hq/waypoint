// What a Wayland tear-off carries through the compositor: the tabs, where they came from, and how to name them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MoveWhat } from '@liminal-hq/waypoint-protocol/generated/MoveWhat';
import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';

/**
 * The opaque JSON a toplevel drag hands the plugin. It travels as the drop's data, and comes back
 * on `toplevel-drag-ended` to both windows of the drag, so whichever window holds the tabs when the
 * drag ends can act on it.
 *
 * `mode` says which window that is. `tabs`: the tabs moved to a window made for the drag, so that
 * window's page decides where they end up. `window`: the dragged tabs are all the window has, so the
 * window itself is dragged and its own page merges it if it is dropped on another window.
 */
export interface TearPayload {
	v: 1;
	mode: 'tabs' | 'window';
	/** What moves, as `moveTabs` names it; tab, group and pair ids keep their identity across windows. */
	what: MoveWhat;
	tabs: TabId[];
	/** What the announcements call it. */
	name: string;
	/** The tabs are pinned where they come from, which a window showing where they would land needs to know. Absent from a payload made before it was carried; read as unpinned. */
	pinned?: boolean;
	/** The window the tabs came from, and where in its strip the first one sat. */
	source: { window: string; index: number };
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null;
}

/** The payload as a drag carried it, or null for anything else (a drop from elsewhere). */
export function parseTearPayload(value: unknown): TearPayload | null {
	if (!isRecord(value) || value.v !== 1) return null;
	const { mode, what, tabs, name, source, pinned } = value;
	if (mode !== 'tabs' && mode !== 'window') return null;
	if (!isRecord(what) || typeof what.kind !== 'string') return null;
	if (!Array.isArray(tabs) || !tabs.every((tab) => typeof tab === 'number')) return null;
	if (typeof name !== 'string') return null;
	if (!isRecord(source) || typeof source.window !== 'string' || typeof source.index !== 'number') {
		return null;
	}
	return {
		v: 1,
		mode,
		what: what as unknown as MoveWhat,
		tabs: tabs as TabId[],
		name,
		pinned: pinned === true,
		source: { window: source.window, index: source.index },
	};
}
