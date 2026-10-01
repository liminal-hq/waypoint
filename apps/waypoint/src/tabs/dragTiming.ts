// The numbers of the tab drag language, in one place so the specification and the behaviour agree
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { DRAG_MOTION_MS } from '../dnd/dragSession';

/** A press becomes a drag after the pointer moves this far (SPEC §13c). */
export const DRAG_START_PX = 4;

/** Dragging this far above or below the strip leaves it: the new-window phase. */
export const TEAR_OFF_PX = 24;

/** Holding over the middle of another tab for this long splits with it. */
export const HOLD_SPLIT_MS = 450;

/** Resting in a slot for this long starts a new group. */
export const REST_GROUP_MS = 800;

/** Slides, rings and brackets last this long, and are instant under Reduce motion (D82). */
export const MOTION_MS = DRAG_MOTION_MS;

/** A hold or rest survives this much pointer drift, so a steady hand is not a moving one. */
export const HOLD_JITTER_PX = 6;

/** The middle share of a tab's width that counts as its body for a split; the rest is its edges. */
export const TAB_BODY_FRACTION = 0.5;

/** How deep into the file area, as a share of its width or height, an edge zone reaches. */
export const EDGE_ZONE_FRACTION = 0.25;
