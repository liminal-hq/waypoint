// Asking the system for administrator rights: connect the elevated helper, then open or leave Administrator Mode
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import type { ConnectionsClient } from '../connections/connectionsClient';
import { connectionErrorText } from '../connections/connectModel';
import { t, tf } from '../i18n/messages';
import { isVfsError } from '../services/vfsClient';
import type { TabsApi } from '../services/tabsApi';
import { elevatedLocation, isElevatedLocation, unelevatedLocation } from './elevatedLocation';
import { elevationStore, type ElevationStore } from './elevationStore';

/** How asking for administrator rights ended. */
export type ElevationOutcome =
	| { kind: 'connected' }
	/** The person closed the prompt from Waypoint's own Cancel. */
	| { kind: 'cancelled' }
	/** The system's prompt was cancelled or refused (KDE reports a cancel as a refusal, so one outcome says both). */
	| { kind: 'refused' }
	| { kind: 'failed'; error: unknown };

type ConnectClient = Pick<ConnectionsClient, 'connect' | 'cancelConnect'>;

/**
 * Connects the elevated helper now, which is what shows the system's prompt, and says so in the
 * store while it waits so the window can offer Cancel. Nothing else ever starts a prompt: not
 * opening a folder, not restoring a session.
 */
export async function connectAdministrator(
	client: ConnectClient,
	location: Location,
	store: ElevationStore = elevationStore,
): Promise<ElevationOutcome> {
	const prompt = {
		cancel: () => {
			client.cancelConnect(location).catch((error: unknown) => {
				console.warn('could not cancel the administrator prompt', error);
			});
		},
	};
	store.getState().begin(prompt);
	try {
		await client.connect(location, null, false);
		return { kind: 'connected' };
	} catch (error) {
		if (isVfsError(error)) {
			if (error.kind === 'cancelled') return { kind: 'cancelled' };
			if (error.kind === 'authFailed') return { kind: 'refused' };
		}
		return { kind: 'failed', error };
	} finally {
		store.getState().end(prompt);
	}
}

/** What to tell the person when asking did not end in Administrator Mode; `null` for a cancel they made themselves. */
export function elevationFailureText(outcome: ElevationOutcome): string | null {
	switch (outcome.kind) {
		case 'connected':
		case 'cancelled':
			return null;
		case 'refused':
			return t('elevation.refused');
		case 'failed': {
			const { error } = outcome;
			if (isVfsError(error) && error.kind === 'unsupported') {
				return tf('elevation.failed', { reason: error.what });
			}
			return connectionErrorText(error);
		}
	}
}

export interface OpenAsAdministratorDeps {
	client: ConnectClient;
	tabs: Pick<TabsApi, 'openTab'>;
	/** The tab the new one opens beside. */
	active: TabSnapshot | undefined;
	/** A short message for the person (the transient notice). */
	say(text: string): void;
	announce(text: string): void;
}

/**
 * Open as Administrator on a local folder: the prompt first, then a new tab at the elevated
 * location. A cancelled prompt leaves nothing behind; a refused or failed one says so.
 */
export async function openAsAdministrator(
	deps: OpenAsAdministratorDeps,
	folder: Location,
): Promise<ElevationOutcome> {
	const admin = elevatedLocation(folder);
	const outcome = await connectAdministrator(deps.client, admin);
	if (outcome.kind === 'connected') {
		await deps.tabs.openTab(admin, deps.active ? { after: deps.active.id } : {});
		deps.announce(tf('elevation.announce.opened', { folder: folder.display }));
		return outcome;
	}
	const words = elevationFailureText(outcome);
	if (words) deps.say(words);
	return outcome;
}

/** Leaving Administrator Mode: the same tab goes to the ordinary form of its folder, so its history is kept. */
export async function leaveAdministrator(deps: {
	tabs: Pick<TabsApi, 'navigate'>;
	active: TabSnapshot | undefined;
	announce(text: string): void;
}): Promise<boolean> {
	const { active } = deps;
	if (!active || !isElevatedLocation(active.location)) return false;
	await deps.tabs.navigate(active.id, unelevatedLocation(active.location));
	deps.announce(t('elevation.announce.left'));
	return true;
}
