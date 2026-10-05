// Tests for the window's copy of the saved connections and login states
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import type { ConnectionEntry } from '@liminal-hq/waypoint-protocol/generated/ConnectionEntry';
import {
	applyChanged,
	applyStatus,
	EMPTY_VIEW,
	anyProtocolOn,
	fromOverview,
	isProtocolOff,
	savedFor,
	schemeOfKey,
	stateOf,
} from './connectionsModel';
import { draft, keyOf, serverLocation } from './fakeConnectionsClient';

function entry(id: string, host: string): ConnectionEntry {
	const d = draft({ host, user: 'me' });
	return {
		connection: { id, ...d },
		label: `me@${host}`,
		key: keyOf(d),
		location: serverLocation(keyOf(d)),
	};
}

describe('the connections view', () => {
	it('starts from an overview and follows changes in revision order', () => {
		const a = entry('c1', 'a.lan');
		const b = entry('c2', 'b.lan');
		let view = fromOverview(EMPTY_VIEW, {
			connections: { revision: 2, connections: [a], recent: [] },
			statuses: [],
		});
		expect(view.loaded).toBe(true);
		view = applyChanged(view, {
			revision: 3,
			changes: [{ id: 'c2', entry: b }],
			order: ['c2', 'c1'],
			recent: null,
		});
		expect(view.connections.map((e) => e.connection.id)).toEqual(['c2', 'c1']);
		// An old change is dropped.
		const same = applyChanged(view, {
			revision: 3,
			changes: [{ id: 'c1', entry: null }],
			order: ['c2'],
			recent: null,
		});
		expect(same).toBe(view);
		view = applyChanged(view, {
			revision: 4,
			changes: [{ id: 'c1', entry: null }],
			order: ['c2'],
			recent: [],
		});
		expect(view.connections).toEqual([b]);
		expect(savedFor(view, 'sftp://me@b.lan')).toBe(b);
	});

	it('keeps an event newer than an overview that arrives late', () => {
		let view = applyChanged(EMPTY_VIEW, {
			revision: 5,
			changes: [{ id: 'c1', entry: entry('c1', 'a.lan') }],
			order: ['c1'],
			recent: null,
		});
		view = fromOverview(view, {
			connections: { revision: 4, connections: [], recent: [] },
			statuses: [],
		});
		expect(view.connections).toHaveLength(1);
	});

	it('tracks each login state by revision', () => {
		let view = applyStatus(EMPTY_VIEW, {
			key: 'sftp://a',
			state: { kind: 'connecting' },
			revision: 1,
		});
		view = applyStatus(view, { key: 'sftp://a', state: { kind: 'connected' }, revision: 2 });
		expect(stateOf(view, 'sftp://a')).toEqual({ kind: 'connected' });
		expect(applyStatus(view, { key: 'sftp://a', state: { kind: 'idle' }, revision: 1 })).toBe(view);
		expect(stateOf(view, 'sftp://other')).toEqual({ kind: 'idle' });
	});
});

describe('the protocols that are on', () => {
	it('shows everything until Rust says which are on', () => {
		expect(EMPTY_VIEW.protocols).toBeNull();
		expect(anyProtocolOn(EMPTY_VIEW)).toBe(true);
		expect(isProtocolOff(EMPTY_VIEW, 'sftp')).toBe(false);
	});

	it('knows a protocol that is off and whether any is on', () => {
		const none = { ...EMPTY_VIEW, protocols: { schemes: [], off: ['sftp', 'dav'] } };
		expect(anyProtocolOn(none)).toBe(false);
		expect(isProtocolOff(none, 'SFTP')).toBe(true);
		const some = { ...EMPTY_VIEW, protocols: { schemes: ['sftp'], off: ['dav'] } };
		expect(anyProtocolOn(some)).toBe(true);
		expect(isProtocolOff(some, 'sftp')).toBe(false);
		expect(isProtocolOff(some, 'smb')).toBe(false);
	});

	it('keeps them when an overview arrives', () => {
		const view = { ...EMPTY_VIEW, protocols: { schemes: ['sftp'], off: [] } };
		const overview = {
			connections: { revision: 1, connections: [], recent: [] },
			statuses: [],
		};
		expect(fromOverview(view, overview).protocols).toEqual({ schemes: ['sftp'], off: [] });
	});

	it('reads the scheme of a login key', () => {
		expect(schemeOfKey('sftp://me@nas.lan')).toBe('sftp');
		expect(schemeOfKey('DAVS://cloud')).toBe('davs');
		expect(schemeOfKey('nonsense')).toBe('');
	});
});
