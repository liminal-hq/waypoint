// The element ids that tie a tab to the panel it controls
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/** One window shows one tab panel, so its id is fixed. */
export const TAB_PANEL_ID = 'wp-tabpanel';

export function tabDomId(tab: number): string {
	return `wp-tab-${tab}`;
}
