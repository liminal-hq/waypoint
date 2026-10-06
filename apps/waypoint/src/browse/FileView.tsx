// The file view for the window's chosen mode: the list or the icon grid over the same listing
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ArchiveOpening } from '../archives/ArchiveOpening';
import { RemoteOpening } from '../connections/RemoteState';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { KeyboardEvent } from 'react';
import { TrashFrame } from '../trash/TrashFrame';
import { isTrashLocation } from '../trash/trashLocation';
import { useTrashActions } from '../trash/trashJobs';
import { TrashUnavailable } from '../trash/TrashUnavailable';
import { GridView } from './GridView';
import { ListingView } from './ListView';
import type { MenuRequest, OpenHandler, OpenInNewHandler } from './useListInteractions';
import type { SessionState } from './useListingSession';
import type { ViewMode } from './viewStore';

interface FileViewProps {
	state: SessionState;
	mode: ViewMode;
	gridSize: number;
	/** Where the listing is, so a Trash that cannot be read is explained as such. */
	location?: Location | undefined;
	/** The file list's accessible name, when more than one list is on screen (the panes of a pair). */
	listLabel?: string | undefined;
	onOpen: OpenHandler;
	onOpenInNewTab: OpenInNewHandler;
	onMenu: (request: MenuRequest) => void;
}

/**
 * Switching modes swaps the view but not the listing: the selection, the focus and the cached
 * pages belong to the listing's session, so they carry across.
 *
 * The Trash is the same view with its own actions: Enter and a double-click say that items cannot
 * be opened there, Delete asks to delete the selection for good, and a strip above the list offers
 * Restore, Delete Permanently and Empty Trash. A Trash that cannot be browsed here is explained
 * where the list would be.
 */
export function FileView({
	state,
	mode,
	gridSize,
	location,
	listLabel,
	onOpen,
	onOpenInNewTab,
	onMenu,
}: FileViewProps) {
	const trashActions = useTrashActions();
	if (state.status === 'error' && state.error.kind === 'unsupported' && isTrashLocation(location)) {
		return <TrashUnavailable reason={state.error.what} />;
	}
	const session = state.status === 'ready' ? state.session : null;
	const inTrash = session?.model.layout === 'trash';
	// Nothing in the Trash is opened: an item is restored first.
	const open: OpenHandler = inTrash ? () => trashActions?.hintOpen() : onOpen;
	const openInNewTab: OpenInNewHandler = inTrash ? () => {} : onOpenInNewTab;
	const shared = {
		state,
		onOpen: open,
		onOpenInNewTab: openInNewTab,
		onMenu,
		announceSelection: false,
		label: listLabel,
	};
	const body =
		mode === 'grid' ? <GridView {...shared} size={gridSize} /> : <ListingView {...shared} />;
	// A server folder whose login is still connecting says so.
	if (state.status === 'opening' && location)
		return (
			<RemoteOpening
				location={location}
				fallback={<ArchiveOpening location={location} fallback={body} />}
			/>
		);
	if (!session || !inTrash) return body;

	const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
		if (event.key !== 'Delete' || event.ctrlKey || event.metaKey || event.altKey) return;
		if (event.nativeEvent.isComposing) return;
		event.preventDefault();
		trashActions?.deletePermanently(session);
	};
	return (
		<TrashFrame session={session} onKeyDown={onKeyDown}>
			{body}
		</TrashFrame>
	);
}
