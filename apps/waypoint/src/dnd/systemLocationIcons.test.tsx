// Verifies the System icon set where files are named by place: the drag stack (a listing row, a Shelf drag, files from outside) and both sides of a conflict
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, render } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ListingSession } from '../browse/useListingSession';
import { TimeFormatProvider } from '../browse/TimeFormatContext';
import { configureSystemIcons } from '../icons/systemIcons';
import { ConflictDialog } from '../ops/ConflictDialog';
import {
	createFakeSystemIconsClient,
	type FakeSystemIcons,
} from '../services/fakeSystemIconsClient';
import { FakeTimeFormatClient } from '../services/fakeTimeFormatClient';
import { fileLocation } from '../services/fakeVfsClient';
import { conflictFor } from '../test/opsHarness';
import { fakeJobSnapshot } from '../trash/fakeTrashClient';
import { createDragSession } from './dragSession';
import { DragStack } from './DragStack';
import type { FileDragSource, FileDropTarget } from './fileDragModel';
import { externalSource } from './nativeDropModel';

const root = document.documentElement;
let fake: FakeSystemIcons;

async function settle(): Promise<void> {
	await act(async () => {
		for (let turn = 0; turn < 8; turn += 1) await Promise.resolve();
	});
}

beforeEach(() => {
	root.dataset.iconTheme = 'system';
	fake = createFakeSystemIconsClient();
	configureSystemIcons(fake);
});
afterEach(() => {
	cleanup();
	document.body.innerHTML = '';
	configureSystemIcons(null);
	delete root.dataset.iconTheme;
});

const files = () => fake.probed.filter((url) => url.startsWith('fake://file/'));

function dragging(source: FileDragSource) {
	const session = createDragSession<FileDragSource, FileDropTarget>({
		startThresholdPx: 0,
		reducedMotion: () => true,
	});
	session.begin(
		{ pointerId: 1, clientX: 0, clientY: 0, element: null, source },
		{
			start: (control) => control.setPill({ text: 'x', kind: 'idle' }),
			move: () => {},
			drop: () => {},
		},
	);
	act(() => {
		window.dispatchEvent(new PointerEvent('pointermove', { clientX: 5, clientY: 5, pointerId: 1 }));
	});
	return session;
}

describe('the drag stack under the System icon set', () => {
	it('draws the pressed row of a listing from its own icon, named by the listing token', async () => {
		const session = dragging({
			session: {} as ListingSession,
			tab: 1,
			handle: 4,
			spec: { kind: 'some', ids: [9] },
			count: 1,
			name: 'Setup.exe',
			groups: ['executable'],
			icons: [{ name: 'Setup.exe', source: { handle: 4, id: 9, modifiedMs: 77 } }],
			folder: { display: '/a', uri: 'file:///a' },
			readOnly: false,
			rightButton: false,
		});
		render(<DragStack session={session} />);
		await settle();
		expect(files()).toEqual(['fake://file/4-9?size=16&scale=1&m=77']);
		expect(fake.registered).toEqual([]);
	});

	it('draws files dragged in from another application by place, up to three cards, and asks Rust once', async () => {
		const dropped = ['/a/one.exe', '/a/two.lnk', '/a/three.txt', '/a/four.exe'].map(fileLocation);
		const session = dragging(externalSource(dropped));
		render(<DragStack session={session} />);
		await settle();
		expect(fake.registered).toHaveLength(1);
		expect(fake.registered[0]!.map((location) => location.uri).sort()).toEqual([
			dropped[0]!.uri,
			dropped[1]!.uri,
		]);
		expect(files()).toHaveLength(2);
		// The type's icon, by the name's own extension, for the file that carries none.
		expect(fake.probed.some((url) => url.includes('fake://ext/txt'))).toBe(true);
	});
});

describe('the conflict dialog under the System icon set', () => {
	it('draws both sides of a program’s conflict from the files, with the times of each in the address', async () => {
		render(
			<TimeFormatProvider client={new FakeTimeFormatClient()}>
				<ConflictDialog
					job={{
						...fakeJobSnapshot(1, { kind: 'copy' }, { state: 'queued' }),
						destination: fileLocation('/dest'),
					}}
					conflicts={[
						conflictFor('Setup.exe', { sourceModifiedMs: 20, existingModifiedMs: 10 }),
						conflictFor('notes.txt'),
					]}
					onContinue={vi.fn()}
					onCancelJob={vi.fn()}
					onLater={vi.fn()}
				/>
			</TimeFormatProvider>,
		);
		await settle();
		expect(fake.registered).toHaveLength(1);
		expect(fake.registered[0]!.map((location) => location.uri).sort()).toEqual([
			'file:///dest/Setup.exe',
			'file:///src/Setup.exe',
		]);
		// Each side has its own number and its own modified time.
		expect(files()).toHaveLength(2);
		expect(new Set(files().map((url) => /file\/(l\d+)\?/.exec(url)?.[1])).size).toBe(2);
		expect(
			files()
				.map((url) => url.split('m=')[1])
				.sort(),
		).toEqual(['10', '20']);
		// The name picks the type's icon of the file that carries no icon of its own.
		expect(fake.probed.some((url) => url.includes('fake://ext/txt'))).toBe(true);
	});
});
