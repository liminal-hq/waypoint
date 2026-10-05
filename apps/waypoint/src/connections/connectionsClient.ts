// What the Connect dialog, the Network section and remote tabs need from the file system plugin's connections
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { AnswerInput } from '@liminal-hq/waypoint-protocol/generated/AnswerInput';
import type { ConnectionDraft } from '@liminal-hq/waypoint-protocol/generated/ConnectionDraft';
import type { ConnectionEntry } from '@liminal-hq/waypoint-protocol/generated/ConnectionEntry';
import type { ConnectionStatus } from '@liminal-hq/waypoint-protocol/generated/ConnectionStatus';
import type { ConnectionSupport } from '@liminal-hq/waypoint-protocol/generated/ConnectionSupport';
import type { ConnectionsChanged } from '@liminal-hq/waypoint-protocol/generated/ConnectionsChanged';
import type { ConnectionsError } from '@liminal-hq/waypoint-protocol/generated/ConnectionsError';
import type { ConnectionsOverview } from '@liminal-hq/waypoint-protocol/generated/ConnectionsOverview';
import type { KeyringUnavailable } from '@liminal-hq/waypoint-protocol/generated/KeyringUnavailable';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { ParsedAddress } from '@liminal-hq/waypoint-protocol/generated/ParsedAddress';
import type { Remembered } from '@liminal-hq/waypoint-protocol/generated/Remembered';
import type { SuggestedServer } from '@liminal-hq/waypoint-protocol/generated/SuggestedServer';
import type { TestedConnection } from '@liminal-hq/waypoint-protocol/generated/TestedConnection';
import type { Unsubscribe } from '../services/vfsClient';

/**
 * The seam between the page and the saved connections and connection manager Rust owns (A81),
 * injected so every view is tested without the plugin. It asks, acts and listens; it never builds
 * an address or keeps a secret. Actions on a login reject with a `VfsError` (the typed reason, as
 * `isVfsError` tells); edits of the saved connections reject with a `ConnectionsRefusal`.
 */
export interface ConnectionsClient {
	list(): Promise<ConnectionsOverview>;
	support(): Promise<ConnectionSupport>;
	suggested(): Promise<SuggestedServer[]>;
	parseAddress(text: string): Promise<ParsedAddress>;
	add(draft: ConnectionDraft): Promise<ConnectionEntry>;
	update(id: string, draft: ConnectionDraft): Promise<ConnectionEntry>;
	duplicate(id: string, name: string): Promise<ConnectionEntry>;
	remove(id: string, forgetLogin: boolean): Promise<KeyringUnavailable | null>;
	move(id: string, to: number): Promise<void>;
	forgetRecent(key: string | null): Promise<void>;
	forgetLogin(location: Location): Promise<KeyringUnavailable | null>;
	/** Connects now with the person's answer; the answer is sent once and never kept here. */
	connect(location: Location, answer?: AnswerInput | null, remember?: boolean): Promise<Remembered>;
	/** Tries a draft without saving it, and says where it opens. */
	test(
		draft: ConnectionDraft,
		answer?: AnswerInput | null,
		remember?: boolean,
	): Promise<TestedConnection>;
	disconnect(location: Location): Promise<void>;
	state(location: Location): Promise<ConnectionStatus | null>;
	onChanged(listener: (change: ConnectionsChanged) => void): Unsubscribe;
	onState(listener: (status: ConnectionStatus) => void): Unsubscribe;
}

/** How an edit of the saved connections is refused. */
export interface ConnectionsRefusal {
	kind: 'connections';
	error: ConnectionsError;
}

export function isConnectionsRefusal(value: unknown): value is ConnectionsRefusal {
	return (
		typeof value === 'object' &&
		value !== null &&
		(value as { kind?: unknown }).kind === 'connections' &&
		typeof (value as { error?: unknown }).error === 'object'
	);
}
