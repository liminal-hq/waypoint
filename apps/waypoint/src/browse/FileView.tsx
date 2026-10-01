// The file view for the window's chosen mode: the list or the icon grid over the same listing
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { GridView } from './GridView';
import { ListingView } from './ListView';
import type { MenuRequest, OpenHandler, OpenInNewHandler } from './useListInteractions';
import type { SessionState } from './useListingSession';
import type { ViewMode } from './viewStore';

interface FileViewProps {
	state: SessionState;
	mode: ViewMode;
	gridSize: number;
	onOpen: OpenHandler;
	onOpenInNewTab: OpenInNewHandler;
	onMenu: (request: MenuRequest) => void;
}

/**
 * Switching modes swaps the view but not the listing: the selection, the focus and the cached
 * pages belong to the listing's session, so they carry across.
 */
export function FileView({ state, mode, gridSize, onOpen, onOpenInNewTab, onMenu }: FileViewProps) {
	const shared = { state, onOpen, onOpenInNewTab, onMenu, announceSelection: false };
	return mode === 'grid' ? <GridView {...shared} size={gridSize} /> : <ListingView {...shared} />;
}
