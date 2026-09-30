// Maps a webview label to the kind of window it hosts
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { WindowKind } from '@liminal-hq/waypoint-protocol/generated/WindowKind';

/** Mirrors `WindowKind::from_label` in `crates/waypoint-protocol`. */
export function windowKindFromLabel(label: string): WindowKind | null {
	switch (label) {
		case 'settings':
			return 'Settings';
		case 'ops':
			return 'Ops';
		case 'tear-ghost':
			return 'TearGhost';
	}
	if (label.startsWith('main-')) return 'Main';
	if (label.startsWith('properties-')) return 'Properties';
	return null;
}
