// An in-memory ConnectionsClient for tests and the demo: saved connections, recent servers and scripted logins
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { AnswerInput } from '@liminal-hq/waypoint-protocol/generated/AnswerInput';
import type { ConnectionDraft } from '@liminal-hq/waypoint-protocol/generated/ConnectionDraft';
import type { ConnectionEntry } from '@liminal-hq/waypoint-protocol/generated/ConnectionEntry';
import type { ConnectionState } from '@liminal-hq/waypoint-protocol/generated/ConnectionState';
import type { ConnectionStatus } from '@liminal-hq/waypoint-protocol/generated/ConnectionStatus';
import type { ConnectionSupport } from '@liminal-hq/waypoint-protocol/generated/ConnectionSupport';
import type { ConnectionsChanged } from '@liminal-hq/waypoint-protocol/generated/ConnectionsChanged';
import type { ConnectionsOverview } from '@liminal-hq/waypoint-protocol/generated/ConnectionsOverview';
import type { KeyringUnavailable } from '@liminal-hq/waypoint-protocol/generated/KeyringUnavailable';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { ParsedAddress } from '@liminal-hq/waypoint-protocol/generated/ParsedAddress';
import type { RecentServer } from '@liminal-hq/waypoint-protocol/generated/RecentServer';
import type { Remembered } from '@liminal-hq/waypoint-protocol/generated/Remembered';
import type { SuggestedServer } from '@liminal-hq/waypoint-protocol/generated/SuggestedServer';
import type { TestedConnection } from '@liminal-hq/waypoint-protocol/generated/TestedConnection';
import type { ProtocolsChanged } from '@liminal-hq/waypoint-protocol/generated/ProtocolsChanged';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import type { Unsubscribe } from '../services/vfsClient';
import type { ConnectionsClient, ConnectionsRefusal } from './connectionsClient';

/** What a scripted server answers a connect with: nothing (it connects) or the error to reject with. */
export type ConnectScript = (
	key: string,
	answer: AnswerInput | null,
) => VfsError | null | Promise<VfsError | null>;

export interface FakeConnectionsOptions {
	connections?: ConnectionDraft[];
	recent?: RecentServer[];
	schemes?: string[];
	off?: string[];
	keyring?: KeyringUnavailable | null;
	suggested?: string[];
	connect?: ConnectScript;
}

/** A draft with every field at its default. */
export function draft(fields: Partial<ConnectionDraft> = {}): ConnectionDraft {
	return {
		name: '',
		scheme: 'sftp',
		host: '',
		port: null,
		user: null,
		auth: 'auto',
		keyFile: null,
		jumpHost: null,
		startFolder: null,
		options: {
			thumbnails: 'off',
			thumbnailMaxMb: null,
			refreshSeconds: null,
			timeoutSeconds: null,
			listingRequests: null,
			transferRequests: null,
			windowKib: null,
			davAuth: null,
			davPreset: null,
			s3Endpoint: null,
			s3Region: null,
			s3PathStyle: null,
			s3Preset: null,
		},
		...fields,
	};
}

/** The login of a draft, the way `waypoint-path` writes it (good enough for the fake's plain names). */
export function keyOf(d: Pick<ConnectionDraft, 'scheme' | 'user' | 'host' | 'port'>): string {
	const user = d.user ? `${encodeURIComponent(d.user)}@` : '';
	const port = d.port ? `:${d.port}` : '';
	return `${d.scheme}://${user}${d.host.toLowerCase()}${port}`;
}

/** A server location of the fake, at `path` under a login. */
export function serverLocation(key: string, path = '/'): Location {
	return { display: `${key}${path}`, uri: `${key}${path}` };
}

function entryOf(id: string, d: ConnectionDraft): ConnectionEntry {
	const key = keyOf(d);
	return {
		connection: { id, ...d },
		label: d.name || key.replace(/^[a-z]+:\/\//, ''),
		key,
		location: serverLocation(key, d.startFolder ?? '/'),
	};
}

function refusal(error: ConnectionsRefusal['error']): ConnectionsRefusal {
	return { kind: 'connections', error };
}

/** An in-memory client; `emitState` lets a test move a login between states. */
export class FakeConnectionsClient implements ConnectionsClient {
	private entries: ConnectionEntry[] = [];
	private recent: RecentServer[];
	private next = 1;
	private revision = 0;
	private stateRevision = 0;
	private readonly states = new Map<string, ConnectionState>();
	private readonly changedListeners = new Set<(change: ConnectionsChanged) => void>();
	private readonly stateListeners = new Set<(status: ConnectionStatus) => void>();
	private readonly protocolListeners = new Set<(change: ProtocolsChanged) => void>();
	readonly calls: { method: string; args: unknown[] }[] = [];
	/** The answers sent with tests and connects, whole (a call records only the kind). */
	readonly answers: AnswerInput[] = [];
	schemes: string[];
	/** Protocols the build has that are turned off (D167). */
	off: string[];
	keyring: KeyringUnavailable | null;
	suggestions: string[];
	script: ConnectScript;

	constructor(options: FakeConnectionsOptions = {}) {
		for (const d of options.connections ?? []) this.entries.push(entryOf(`c${this.next++}`, d));
		this.recent = options.recent ?? [];
		this.schemes = options.schemes ?? ['sftp'];
		this.off = options.off ?? [];
		this.keyring = options.keyring ?? null;
		this.suggestions = options.suggested ?? [];
		this.script = options.connect ?? (() => null);
	}

	private record(method: string, ...args: unknown[]) {
		this.calls.push({ method, args });
	}

	private changed(change: Omit<ConnectionsChanged, 'revision'>) {
		this.revision += 1;
		const event = { ...change, revision: this.revision };
		for (const listener of this.changedListeners) listener(event);
	}

	/** Moves a login to `state` and tells the listeners. */
	emitState(key: string, state: ConnectionState) {
		this.states.set(key, state);
		this.stateRevision += 1;
		const status = { key, state, revision: this.stateRevision };
		for (const listener of this.stateListeners) listener(status);
	}

	async list(): Promise<ConnectionsOverview> {
		return {
			connections: {
				revision: this.revision,
				connections: [...this.entries],
				recent: [...this.recent],
			},
			statuses: [...this.states].map(([key, state]) => ({
				key,
				state,
				revision: this.stateRevision,
			})),
		};
	}

	async support(): Promise<ConnectionSupport> {
		return { schemes: [...this.schemes], off: [...this.off], keyring: this.keyring };
	}

	async suggested(): Promise<SuggestedServer[]> {
		return this.suggestions.map((alias) => ({
			alias,
			location: serverLocation(`sftp://${alias}`),
		}));
	}

	async parseAddress(text: string): Promise<ParsedAddress> {
		this.record('parseAddress', text);
		const s3 = /^\s*s3:\/\/([^/?\s]+)(\/[^?\s]*)?(?:\?endpoint=(\S+))?\s*$/i.exec(text);
		if (s3) {
			if (this.off.includes('s3')) throw { kind: 'protocolOff', scheme: 's3' } satisfies VfsError;
			if (!this.schemes.includes('s3'))
				throw { kind: 'unsupported', what: 's3' } satisfies VfsError;
			const path = s3[2] && s3[2] !== '/' ? s3[2].replace(/\/+$/, '') : null;
			const d = draft({
				scheme: 's3',
				host: s3[1] ?? '',
				startFolder: path,
				options: {
					...draft().options,
					s3Endpoint: s3[3] ? decodeURIComponent(s3[3]) : null,
				},
			});
			const key = keyOf(d);
			return {
				draft: d,
				location: serverLocation(key, path ?? '/'),
				key,
				passwordDropped: false,
			};
		}
		const match =
			/^\s*([a-zA-Z]+):\/\/(?:([^:@/]+)(?::([^@/]*))?@)?([^:/]+)(?::(\d+))?(\/[^\s]*)?\s*$/.exec(
				text,
			);
		if (!match) throw { kind: 'invalidLocation', input: text } satisfies VfsError;
		const scheme = (match[1] ?? '').toLowerCase();
		if (this.off.includes(scheme)) throw { kind: 'protocolOff', scheme } satisfies VfsError;
		if (!this.schemes.includes(scheme))
			throw { kind: 'unsupported', what: scheme } satisfies VfsError;
		const port = match[5] && match[5] !== '22' ? Number(match[5]) : null;
		const path = match[6] && match[6] !== '/' ? match[6].replace(/\/+$/, '') : null;
		const d = draft({
			scheme,
			user: match[2] ?? null,
			host: (match[4] ?? '').toLowerCase(),
			port,
			startFolder: path,
		});
		const key = keyOf(d);
		return {
			draft: d,
			location: serverLocation(key, path ?? '/'),
			key,
			passwordDropped: match[3] !== undefined,
		};
	}

	private check(d: ConnectionDraft) {
		if (!d.host.trim() || /\s/.test(d.host.trim()))
			throw refusal({ kind: 'draft', error: { kind: 'host' } });
		if (
			!this.schemes.includes(d.scheme) &&
			!['sftp', 'smb', 'dav', 'davs', 's3'].includes(d.scheme)
		)
			throw refusal({ kind: 'draft', error: { kind: 'scheme', scheme: d.scheme } });
		if (d.port === 0) throw refusal({ kind: 'draft', error: { kind: 'port' } });
		if (d.startFolder && !d.startFolder.startsWith('/'))
			throw refusal({ kind: 'draft', error: { kind: 'startFolder' } });
		// A bucket keeps its letter case; a host name does not.
		return {
			...d,
			name: d.name.trim(),
			host: d.scheme === 's3' ? d.host.trim() : d.host.trim().toLowerCase(),
		};
	}

	async add(d: ConnectionDraft): Promise<ConnectionEntry> {
		this.record('add', d);
		const entry = entryOf(`c${this.next++}`, this.check(d));
		this.entries.push(entry);
		this.changed({
			changes: [{ id: entry.connection.id, entry }],
			order: this.entries.map((e) => e.connection.id),
			recent: null,
		});
		return entry;
	}

	async update(id: string, d: ConnectionDraft): Promise<ConnectionEntry> {
		this.record('update', id, d);
		const at = this.entries.findIndex((e) => e.connection.id === id);
		if (at < 0) throw refusal({ kind: 'notFound', id });
		const entry = entryOf(id, this.check(d));
		this.entries[at] = entry;
		this.changed({ changes: [{ id, entry }], order: null, recent: null });
		return entry;
	}

	async duplicate(id: string, name: string): Promise<ConnectionEntry> {
		this.record('duplicate', id, name);
		const at = this.entries.findIndex((e) => e.connection.id === id);
		if (at < 0) throw refusal({ kind: 'notFound', id });
		const original = this.entries[at];
		if (!original) throw refusal({ kind: 'notFound', id });
		const { id: _id, ...fields } = original.connection;
		const entry = entryOf(`c${this.next++}`, { ...fields, name });
		this.entries.splice(at + 1, 0, entry);
		this.changed({
			changes: [{ id: entry.connection.id, entry }],
			order: this.entries.map((e) => e.connection.id),
			recent: null,
		});
		return entry;
	}

	async remove(id: string, forgetLogin: boolean): Promise<KeyringUnavailable | null> {
		this.record('remove', id, forgetLogin);
		const at = this.entries.findIndex((e) => e.connection.id === id);
		if (at < 0) throw refusal({ kind: 'notFound', id });
		this.entries.splice(at, 1);
		this.changed({
			changes: [{ id, entry: null }],
			order: this.entries.map((e) => e.connection.id),
			recent: null,
		});
		return forgetLogin ? this.keyring : null;
	}

	async move(id: string, to: number): Promise<void> {
		this.record('move', id, to);
		const at = this.entries.findIndex((e) => e.connection.id === id);
		if (at < 0) throw refusal({ kind: 'notFound', id });
		const [entry] = this.entries.splice(at, 1);
		if (!entry) return;
		this.entries.splice(Math.min(to, this.entries.length), 0, entry);
		this.changed({ changes: [], order: this.entries.map((e) => e.connection.id), recent: null });
	}

	async forgetRecent(key: string | null): Promise<void> {
		this.record('forgetRecent', key);
		this.recent = key === null ? [] : this.recent.filter((server) => server.key !== key);
		this.changed({ changes: [], order: null, recent: [...this.recent] });
	}

	async forgetLogin(location: Location): Promise<KeyringUnavailable | null> {
		this.record('forgetLogin', location);
		return this.keyring;
	}

	private keyOfLocation(location: Location): string {
		const match = /^([a-z]+:\/\/[^/]+)/.exec(location.uri);
		if (!match) throw { kind: 'invalidLocation', input: location.uri } satisfies VfsError;
		return match[1] ?? '';
	}

	private async run(
		key: string,
		answer: AnswerInput | null,
		remember: boolean,
	): Promise<Remembered> {
		this.emitState(key, { kind: 'connecting' });
		const error = await this.script(key, answer);
		if (error) {
			this.emitState(key, { kind: 'failed', error });
			throw error;
		}
		this.emitState(key, { kind: 'connected' });
		const credential =
			answer && ['password', 'passphrase', 'accessKey'].includes(answer.kind) && remember;
		if (!credential) return { kind: 'no' };
		return this.keyring ? { kind: 'sessionOnly', why: this.keyring } : { kind: 'kept' };
	}

	async connect(
		location: Location,
		answer: AnswerInput | null = null,
		remember = false,
	): Promise<Remembered> {
		this.record('connect', location, answer?.kind ?? null, remember);
		const scheme = /^([a-z]+):\/\//.exec(location.uri)?.[1] ?? '';
		if (this.off.includes(scheme)) throw { kind: 'protocolOff', scheme } satisfies VfsError;
		return this.run(this.keyOfLocation(location), answer, remember);
	}

	async test(
		d: ConnectionDraft,
		answer: AnswerInput | null = null,
		remember = false,
	): Promise<TestedConnection> {
		this.record('test', d, answer?.kind ?? null, remember);
		if (answer) this.answers.push(answer);
		const checked = this.check(d);
		const key = keyOf(checked);
		const remembered = await this.run(key, answer, remember);
		return { remembered, key, location: serverLocation(key, checked.startFolder ?? '/') };
	}

	async nextcloudAddress(server: string, user: string): Promise<string> {
		this.record('nextcloudAddress', server, user);
		const match = /^(?:(https?|davs?):\/\/)?([^/@\s]+)((?:\/[^/\s]+)*)\/?$/.exec(server.trim());
		if (!match || !user) throw { kind: 'invalidLocation', input: server } satisfies VfsError;
		const scheme = match[1] === 'http' || match[1] === 'dav' ? 'dav' : 'davs';
		const id = encodeURIComponent(user);
		return `${scheme}://${id}@${match[2]}${match[3] ?? ''}/remote.php/dav/files/${id}`;
	}

	async disconnect(location: Location): Promise<void> {
		this.record('disconnect', location);
		this.emitState(this.keyOfLocation(location), { kind: 'idle' });
	}

	async state(location: Location): Promise<ConnectionStatus | null> {
		if (!/^[a-z]+:\/\/[^/]/.test(location.uri) || location.uri.startsWith('file:')) return null;
		const key = this.keyOfLocation(location);
		return { key, state: this.states.get(key) ?? { kind: 'idle' }, revision: 0 };
	}

	onChanged(listener: (change: ConnectionsChanged) => void): Unsubscribe {
		this.changedListeners.add(listener);
		return () => this.changedListeners.delete(listener);
	}

	onState(listener: (status: ConnectionStatus) => void): Unsubscribe {
		this.stateListeners.add(listener);
		return () => this.stateListeners.delete(listener);
	}

	onProtocols(listener: (change: ProtocolsChanged) => void): Unsubscribe {
		this.protocolListeners.add(listener);
		return () => this.protocolListeners.delete(listener);
	}

	/** Turns the protocols in `on` on and the other `known` ones off, and tells the listeners. */
	setProtocols(on: string[], known: string[] = [...this.schemes, ...this.off]) {
		this.schemes = known.filter((scheme) => on.includes(scheme));
		this.off = known.filter((scheme) => !on.includes(scheme));
		const change = { schemes: [...this.schemes], off: [...this.off] };
		for (const listener of this.protocolListeners) listener(change);
	}
}
