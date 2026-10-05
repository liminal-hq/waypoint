// What a remote location's state is called and what it offers: the words for each connection state and error
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ConnectionState } from '@liminal-hq/waypoint-protocol/generated/ConnectionState';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import { t, type MessageId } from '../i18n/messages';

/** The errors that are about the connection rather than the folder asked for (A80). */
const CONNECTION_KINDS: ReadonlySet<VfsError['kind']> = new Set([
	'disconnected',
	'unreachable',
	'timeout',
	'authRequired',
	'authFailed',
	'hostKeyUnknown',
	'hostKeyChanged',
	'certificateUntrusted',
	'clockSkew',
]);

export function isConnectionError(error: VfsError): boolean {
	return CONNECTION_KINDS.has(error.kind);
}

/** One word-group per state, so a dot's colour is never the only sign (accessibility.md). */
export type StateTone =
	'idle' | 'connecting' | 'connected' | 'offline' | 'signIn' | 'trust' | 'error';

export function stateTone(state: ConnectionState): StateTone {
	switch (state.kind) {
		case 'idle':
			return 'idle';
		case 'connecting':
			return 'connecting';
		case 'connected':
			return 'connected';
		case 'failed':
			switch (state.error.kind) {
				case 'disconnected':
				case 'unreachable':
				case 'timeout':
					return 'offline';
				case 'authRequired':
				case 'authFailed':
					return 'signIn';
				case 'hostKeyUnknown':
				case 'hostKeyChanged':
				case 'certificateUntrusted':
					return 'trust';
				default:
					return 'error';
			}
	}
}

const TONE_WORDS: Record<StateTone, MessageId> = {
	idle: 'remote.state.idle',
	connecting: 'remote.state.connecting',
	connected: 'remote.state.connected',
	offline: 'remote.state.offline',
	signIn: 'remote.state.signIn',
	trust: 'remote.state.trust',
	error: 'remote.state.error',
};

/** The state in words, for the Network section, the tab and announcements. */
export function stateWords(state: ConnectionState): string {
	return t(TONE_WORDS[stateTone(state)]);
}

/** What the remote state of a folder offers. */
export type RemoteAction = 'reconnect' | 'signIn' | 'review';

export interface RemoteStateText {
	title: MessageId;
	detail: MessageId;
	action: RemoteAction;
}

const UNREACHABLE: Record<string, MessageId> = {
	nameNotResolved: 'remote.unreachable.nameNotResolved',
	refused: 'remote.unreachable.refused',
	noRoute: 'remote.unreachable.noRoute',
	offline: 'remote.unreachable.offline',
};

/** The title, the detail and the action of a folder that could not be shown because of its connection. */
export function remoteStateText(error: VfsError): RemoteStateText {
	switch (error.kind) {
		case 'disconnected':
			return {
				title: 'remote.disconnected.title',
				detail: 'remote.disconnected.detail',
				action: 'reconnect',
			};
		case 'unreachable':
			return {
				title: 'remote.unreachable.title',
				detail: UNREACHABLE[error.reason] ?? 'remote.unreachable.offline',
				action: 'reconnect',
			};
		case 'timeout':
			return {
				title: 'remote.timeout.title',
				detail: 'remote.timeout.detail',
				action: 'reconnect',
			};
		case 'authRequired':
			return { title: 'remote.signIn.title', detail: 'remote.signIn.detail', action: 'signIn' };
		case 'authFailed':
			return { title: 'remote.signIn.title', detail: 'remote.signIn.refused', action: 'signIn' };
		case 'hostKeyUnknown':
			return { title: 'remote.hostKey.title', detail: 'remote.hostKey.detail', action: 'review' };
		case 'hostKeyChanged':
			return {
				title: 'remote.hostKeyChanged.title',
				detail: 'remote.hostKeyChanged.detail',
				action: 'review',
			};
		case 'certificateUntrusted':
			return {
				title: 'remote.certificate.title',
				detail: 'remote.certificate.detail',
				action: 'review',
			};
		case 'clockSkew':
			return {
				title: 'remote.clockSkew.title',
				detail: 'remote.clockSkew.detail',
				action: 'reconnect',
			};
		default:
			return { title: 'remote.error.title', detail: 'remote.error.detail', action: 'reconnect' };
	}
}
