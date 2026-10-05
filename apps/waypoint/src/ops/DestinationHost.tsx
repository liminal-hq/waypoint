// Shows the destination dialog when something asks for a folder: mount one in each window that can copy or move
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useCallback, useEffect, useMemo } from 'react';
import { useStore } from 'zustand';
import { useVfsClient } from '../browse/VfsClientContext';
import { useConnectionsView } from '../connections/ConnectionsContext';
import { t } from '../i18n/messages';
import type { Location } from '../services/opsClient';
import { LABELS } from '../sidebar/PlaceList';
import { usePlacesClient } from '../sidebar/PlacesClientContext';
import { usePlaces } from '../sidebar/usePlaces';
import { useTabsSnapshot } from '../tabs/TabsContext';
import { locationLabel } from '../tabs/tabTitle';
import { isOverviewLocation } from '../overview/overviewLocation';
import { isTrashLocation } from '../trash/trashLocation';
import { normaliseUri } from './clipboardRules';
import {
	DestinationDialog,
	type DestinationChoice,
	type DestinationChoices,
} from './DestinationDialog';
import { recentDestinations, type RecentDestinations } from './destinationModel';
import { attachHost, destinationStore, type DestinationStore } from './destinationStore';
import { createRequest, waitForJob } from './fileCommands';
import { useOps } from './OpsContext';
import { errorText } from './jobText';

export interface DestinationHostProps {
	/** The store the dialog follows; the window's own by default. */
	store?: DestinationStore;
	recent?: RecentDestinations;
}

/** The first of each folder, so a folder that is a place, a favourite and an open tab is offered once per list. */
function unique(choices: DestinationChoice[]): DestinationChoice[] {
	const seen = new Set<string>();
	return choices.filter((choice) => {
		const key = normaliseUri(choice.location.uri);
		if (seen.has(key)) return false;
		seen.add(key);
		return true;
	});
}

export function DestinationHost({
	store = destinationStore,
	recent = recentDestinations,
}: DestinationHostProps) {
	const request = useStore(store, (state) => state.request);
	const vfs = useVfsClient();
	const places = usePlaces(usePlacesClient());
	const snapshot = useTabsSnapshot();
	const ops = useOps();
	const savedServers = useConnectionsView((view) => view.connections);
	const recentServers = useConnectionsView((view) => view.recent);

	useEffect(() => attachHost(store), [store]);

	const createFolder = useCallback(
		async (parent: Location): Promise<Location> => {
			if (!ops) throw new Error(t('files.noQueue'));
			const name = t('files.default.folder');
			const id = await ops.handle.submitJob(
				createRequest('createFolder', parent, name, ops.windowLabel),
			);
			const job = await waitForJob(ops.handle, id);
			if (job?.state.state === 'failed') throw new Error(errorText(job.state.error));
			if (job?.state.state !== 'done') throw new Error(t('files.noQueue'));
			// The planner took the first free name, which the job reports.
			return vfs.parseLocation(job.sources.first ?? name, parent);
		},
		[ops, vfs],
	);

	const choices = useMemo<DestinationChoices>(
		() => ({
			places: (places?.places ?? [])
				.filter((place) => place.kind !== 'trash' && place.kind !== 'overview')
				.map((place) => ({ label: t(LABELS[place.kind]), location: place.location })),
			favourites: (places?.favourites ?? []).map((favourite) => ({
				label: favourite.label,
				location: favourite.location,
			})),
			servers: unique([
				...savedServers.map((entry) => ({ label: entry.label, location: entry.location })),
				...recentServers.map((server) => ({
					label: server.location.display,
					location: server.location,
				})),
			]),
			tabs: unique(
				(snapshot?.tabs ?? [])
					.filter((tab) => !isTrashLocation(tab.location) && !isOverviewLocation(tab.location))
					.map((tab) => ({ label: locationLabel(tab.location), location: tab.location })),
			),
			recent: recent.list().map((location) => ({ label: location.display, location })),
		}),
		[request, recent, places, snapshot, savedServers, recentServers],
	);

	if (!request) return null;
	const { options } = request;
	// A copy starts where the last copy or move went, otherwise where the items are. A move starts
	// empty (the recent folders are listed to pick from): Enter must never move the selection to a
	// folder the person has not named this time.
	const startEmpty = options.forbidOrigin === true && options.initial === undefined;
	const initial = options.initial ?? recent.list()[0] ?? options.base;
	return (
		<DestinationDialog
			options={{ ...options, initial, startEmpty }}
			vfs={vfs}
			choices={choices}
			createFolder={ops ? createFolder : undefined}
			onConfirm={(destination) => {
				recent.remember(destination);
				request.resolve(destination);
			}}
			onCancel={() => request.resolve(null)}
		/>
	);
}
