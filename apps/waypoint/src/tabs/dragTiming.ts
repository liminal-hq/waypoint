// The numbers of the tab drag language, in one place so the specification and the behaviour agree
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { DRAG_MOTION_MS } from '../dnd/dragSession';

/** A press becomes a drag after the pointer moves this far (SPEC §13c). */
export const DRAG_START_PX = 4;

/** Out of the window, never this: below the strip a drag stops arming holds or joining chips this far from it. */
export const STRIP_BAND_PX = 24;

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

/** A pointer riding the border of a split region or the file area stays in the region it was in this far out. */
export const SPLIT_ZONE_SLACK_PX = 6;
