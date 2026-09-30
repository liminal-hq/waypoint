// Verifies live updates with synthetic patch sequences: after any run of adds, removes and updates the cache agrees with the listing
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { ListingEvent } from '@liminal-hq/waypoint-protocol/generated/ListingEvent';
import type { PatchOp } from '@liminal-hq/waypoint-protocol/generated/PatchOp';
import type { SortSpec } from '@liminal-hq/waypoint-protocol/generated/SortSpec';
import { describe, expect, it, vi } from 'vitest';
import { FakeVfsClient, makeEntry } from '../services/fakeVfsClient';
import { FOLDER, seededRandom } from '../test/browseHarness';
import { ListingModel, openListingModel, PAGE_SIZE } from './listingModel';
import { createBrowseStore } from './browseStore';
import { isReset, mapPosition } from './patch';
import { isSelected, selectedCount } from './selection';

// Each sequence applies dozens of edits to a listing of up to 700 entries. They take under a second
// locally but several on a slow CI runner, well inside this limit and past the 5 second default's
// comfort zone.
const SEQUENCE_TIMEOUT_MS = 30_000;

const SORTS: SortSpec[] = [
	{ key: 'name', descending: false, directoriesFirst: true },
	{ key: 'name', descending: true, directoriesFirst: false },
	{ key: 'size', descending: false, directoriesFirst: true },
	{ key: 'modified', descending: true, directoriesFirst: false },
];

interface Run {
	client: FakeVfsClient;
	model: ListingModel;
	/** Every `changed` event the client emitted since the last call. */
	events(): Array<Extract<ListingEvent, { kind: 'changed' }>>;
	/** The listing's true view, read from the client rather than the cache. */
	truth(): Promise<Entry[]>;
	mutate(): void;
}

async function start(seed: number, count: number, sort: SortSpec): Promise<Run> {
	const random = seededRandom(seed);
	const client = new FakeVfsClient();
	let nextId = 1;
	const fresh = (): Entry => {
		const id = nextId++;
		const name = `${random() < 0.15 ? 'dir' : 'item'}-${Math.floor(random() * 1e6)}-${id}`;
		return makeEntry(id, name, {
			kind: name.startsWith('dir') ? 'directory' : 'file',
			size: name.startsWith('dir') ? null : Math.floor(random() * 1e6),
			modifiedMs: 1_600_000_000_000 + Math.floor(random() * 1e9),
		});
	};
	let current: Entry[] = Array.from({ length: count }, fresh);
	client.setFolder(FOLDER, current);
	const model = await openListingModel(client, FOLDER, { sort });
	await model.readRange(0, model.count);

	const seen: Array<Extract<ListingEvent, { kind: 'changed' }>> = [];
	client.onListingEvent((event) => {
		if (event.kind === 'changed') seen.push(event);
	});
	const pick = (n: number) => {
		const ids = new Set<number>();
		for (let i = 0; i < n && current.length > 0; i++) {
			ids.add(current[Math.floor(random() * current.length)]!.id);
		}
		return [...ids];
	};

	return {
		client,
		model,
		events: () => seen.splice(0),
		truth: () => client.getRange(model.handle, 0, model.count + 10),
		mutate() {
			const roll = random();
			if (roll < 0.3) {
				const added = Array.from({ length: 1 + Math.floor(random() * 6) }, fresh);
				current = [...current, ...added];
				client.addEntries(FOLDER, added);
			} else if (roll < 0.35) {
				// A burst large enough to cross page boundaries.
				const added = Array.from({ length: 200 + Math.floor(random() * 400) }, fresh);
				current = [...current, ...added];
				client.addEntries(FOLDER, added);
			} else if (roll < 0.7) {
				const gone = pick(1 + Math.floor(random() * 8));
				current = current.filter((entry) => !gone.includes(entry.id));
				client.removeEntries(FOLDER, gone);
			} else if (roll < 0.75) {
				const gone = pick(150);
				current = current.filter((entry) => !gone.includes(entry.id));
				client.removeEntries(FOLDER, gone);
			} else {
				// Mostly a change that keeps order (touching another column's data); sometimes
				// one that moves the entry, which the client can only report as a reset.
				const ids = pick(1 + Math.floor(random() * 5));
				const changes = new Map<number, Partial<Entry>>();
				for (const id of ids) {
					changes.set(
						id,
						random() < 0.2
							? { name: `renamed-${Math.floor(random() * 1e6)}-${id}` }
							: { modifiedMs: 1_700_000_000_000 + Math.floor(random() * 1e9), group: 'other' },
					);
				}
				current = current.map((entry) =>
					changes.has(entry.id) ? { ...entry, ...changes.get(entry.id) } : entry,
				);
				client.updateEntries(FOLDER, changes);
			}
		},
	};
}

const insertedBy = (ops: PatchOp[]) =>
	ops.reduce((total, op) => total + (op.kind === 'insert' ? op.count : 0), 0);

describe('patch sequences against the fake client', () => {
	const seeds = Array.from({ length: 24 }, (_, i) => i + 1);

	it.each(seeds)(
		'keeps the cache equal to the listing through random edits (seed %i)',
		async (seed) => {
			const sort = SORTS[seed % SORTS.length]!;
			const run = await start(seed, 30 + ((seed * 97) % 700), sort);
			for (let step = 0; step < 30; step++) {
				run.mutate();
				const events = run.events();
				const ops = events.flatMap((event) => event.ops);
				const truth = await run.truth();
				const { model } = run;
				const where = `seed ${seed}, step ${step}`;

				// The count follows the patch.
				expect(model.count, where).toBe(truth.length);
				expect(model.revision, where).toBe(events.at(-1)?.revision ?? model.revision);

				// Straight after the patch, before any refetch, the cache never claims something false:
				// every entry it calls fresh is the listing's entry at that position.
				for (let position = 0; position < model.count; position++) {
					if (model.hasFresh(position)) {
						expect(model.entryAt(position), `${where}, position ${position}`).toEqual(
							truth[position],
						);
					}
				}

				if (!isReset(ops)) {
					// Without a reset every cached entry is the right one for its position, stale or
					// not, and exactly the inserted positions are missing: nothing else was thrown away.
					let missing = 0;
					for (let position = 0; position < model.count; position++) {
						const cached = model.entryAt(position);
						if (!cached) missing++;
						else expect(cached.id, `${where}, position ${position}`).toBe(truth[position]!.id);
					}
					expect(missing, where).toBe(insertedBy(ops));
				}

				// And once the view asks for what is stale or missing, the cache equals the listing.
				await model.readRange(0, model.count);
				const rows = Array.from({ length: model.count }, (_, i) => model.entryAt(i));
				expect(rows, where).toEqual(truth.slice(0, model.count));
				expect(model.staleCount, where).toBe(0);
			}
			run.model.dispose();
		},
		SEQUENCE_TIMEOUT_MS,
	);

	it('refetches only the page an insert landed in, not the pages it shifted', async () => {
		const run = await start(99, 2000, SORTS[0]!);
		const getRange = vi.spyOn(run.client, 'getRange');
		// Sorts between two existing entries somewhere in the middle of the view.
		const middle = run.model.entryAt(1000)!;
		run.client.addEntries(FOLDER, [makeEntry(900_001, `${middle.name}-x`)]);
		run.model.ensure(0, run.model.count - 1);
		await vi.waitFor(() => expect(run.model.staleCount).toBe(0));
		await run.model.readRange(0, run.model.count);
		const pages = getRange.mock.calls.filter(
			(call) => call[0] === run.model.handle && call[2] === PAGE_SIZE,
		);
		expect(pages.length).toBeLessThanOrEqual(2);
		expect(run.model.count).toBe(2001);
	});

	it('keeps updated entries on screen, stale, and refetches only their page', async () => {
		const run = await start(7, 1500, SORTS[2]!);
		const target = run.model.entryAt(700)!;
		const getRange = vi.spyOn(run.client, 'getRange');
		run.client.updateEntries(FOLDER, new Map([[target.id, { modifiedMs: 1 }]]));
		expect(run.model.entryAt(700)).toEqual(target);
		expect(run.model.hasFresh(700)).toBe(false);
		expect(run.model.staleCount).toBe(1);
		run.model.ensure(0, 1499);
		await vi.waitFor(() => expect(run.model.hasFresh(700)).toBe(true));
		expect(run.model.entryAt(700)!.modifiedMs).toBe(1);
		expect(getRange).toHaveBeenCalledTimes(1);
	});

	it('keeps every entry on screen through a reset, then replaces them', async () => {
		const run = await start(3, 600, SORTS[0]!);
		const before = Array.from({ length: 600 }, (_, i) => run.model.entryAt(i));
		const renamed = run.model.entryAt(10)!;
		run.client.updateEntries(FOLDER, new Map([[renamed.id, { name: '0-renamed-first' }]]));
		expect(run.events().some((event) => isReset(event.ops))).toBe(true);
		expect(Array.from({ length: 600 }, (_, i) => run.model.entryAt(i))).toEqual(before);
		expect(run.model.staleCount).toBe(600);
		await run.model.readRange(0, 600);
		expect(run.model.staleCount).toBe(0);
		expect(run.model.entryAt(10)!.id).not.toBe(renamed.id);
	});

	it('falls back to a full refresh when an event is missed', async () => {
		const run = await start(5, 300, SORTS[0]!);
		run.client.addEntries(FOLDER, [makeEntry(800_000, 'zzz-one')]);
		// The next change arrives as revision + 2, as if one event had been lost on the way.
		const skipped: ListingEvent = {
			kind: 'changed',
			handle: run.model.handle,
			revision: run.model.revision + 2,
			count: 301,
			ops: [{ kind: 'insert', at: 0, count: 1 }],
		};
		run.model.applyEvent(skipped);
		expect(run.model.count).toBe(301);
		expect(run.model.staleCount).toBeGreaterThan(0);
	});

	it('distrusts a patch whose arithmetic does not match the announced count', async () => {
		const run = await start(6, 100, SORTS[0]!);
		run.model.applyEvent({
			kind: 'changed',
			handle: run.model.handle,
			revision: run.model.revision + 1,
			count: 90,
			ops: [{ kind: 'remove', at: 0, count: 3 }],
		});
		expect(run.model.count).toBe(90);
		expect(run.model.staleCount).toBe(90);
	});
});

describe('following the top visible entry through patches', () => {
	const seeds = Array.from({ length: 12 }, (_, i) => i + 100);

	it.each(seeds)(
		'maps a tracked position to where its entry went (seed %i)',
		async (seed) => {
			const run = await start(seed, 400, SORTS[seed % 2]!);
			const random = seededRandom(seed * 31);
			for (let step = 0; step < 25; step++) {
				const before = await run.truth();
				const tracked = Math.floor(random() * before.length);
				const id = before[tracked]!.id;
				run.mutate();
				const events = run.events();
				const ops = events.flatMap((event) => event.ops);
				if (isReset(ops)) continue;
				const after = await run.truth();
				const now = after.findIndex((entry) => entry.id === id);
				const mapped = mapPosition(tracked, ops);
				if (now === -1) expect(mapped.removed, `seed ${seed}, step ${step}`).toBe(true);
				else
					expect(mapped, `seed ${seed}, step ${step}`).toEqual({ position: now, removed: false });
			}
		},
		SEQUENCE_TIMEOUT_MS,
	);
});

describe('selection through patches', () => {
	it('keeps selected ids across inserts, removals and updates, and drops the removed ones', async () => {
		const run = await start(11, 500, SORTS[0]!);
		const store = createBrowseStore(run.model);
		const rows = await run.model.readRange(0, 500);
		const chosen = [rows[3]!, rows[40]!, rows[41]!, rows[300]!];
		store.getState().click(3, chosen[0]!.id);
		for (const [i, entry] of chosen.slice(1).entries()) {
			store.getState().toggleAt([40, 41, 300][i]!, entry.id);
		}
		expect(selectedCount(store.getState().selection, run.model.count)).toBe(4);

		run.client.addEntries(FOLDER, [
			makeEntry(700_001, 'aaa-first'),
			makeEntry(700_002, 'bbb-second'),
		]);
		run.client.updateEntries(FOLDER, new Map([[chosen[1]!.id, { modifiedMs: 5 }]]));
		run.client.removeEntries(FOLDER, [chosen[2]!.id, rows[7]!.id]);

		const { selection } = store.getState();
		expect(isSelected(selection, chosen[0]!.id)).toBe(true);
		expect(isSelected(selection, chosen[1]!.id)).toBe(true);
		expect(isSelected(selection, chosen[2]!.id)).toBe(false);
		expect(isSelected(selection, chosen[3]!.id)).toBe(true);
		expect(selectedCount(selection, run.model.count)).toBe(3);
	});
});
