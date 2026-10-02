// Verifies the clipboard service: Rust's clipboard mirrored by revision, shared by every window, and kept level with the system file clipboard
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import { createFakeOpsClient } from '../services/fakeOpsClient';
import { FakeOsClipboardClient } from '../services/fakeOsClipboardClient';
import { FakeVfsClient } from '../services/fakeVfsClient';
import type { Clipboard, OpsClient, SelectionSpec } from '../services/opsClient';
import { child } from '../test/clipboardHarness';
import { FOLDER } from '../test/browseHarness';
import { cutNames } from './clipboardRules';
import { createClipboardService, type ClipboardService } from './clipboardService';

const spec: SelectionSpec = { kind: 'some', ids: [1, 2] };
const OTHER_FILES = { uris: ['file:///home/elsewhere/z.txt'], cut: false };

function setup(options: { os?: FakeOsClipboardClient | null; focus?: EventTarget | null } = {}) {
	const client = createFakeOpsClient({
		resolveSelection: () => [child(FOLDER, 'a.txt'), child(FOLDER, 'b.txt')],
	});
	const os = options.os === undefined ? new FakeOsClipboardClient() : options.os;
	const vfs = new FakeVfsClient();
	const made: ClipboardService[] = [];
	const window = () => {
		const service = createClipboardService({
			client,
			vfs,
			os,
			focusTarget: options.focus ?? null,
		});
		made.push(service);
		return service;
	};
	return { client, os, vfs, window, dispose: () => made.forEach((service) => service.dispose()) };
}

const board = (service: ClipboardService): Clipboard => service.store.getState().clipboard;

describe('the mirror', () => {
	it('starts empty and follows what Rust says by revision, ignoring an older event after a newer one', async () => {
		const listeners = new Set<(c: Clipboard) => void>();
		const client = {
			getClipboard: async () => ({ mode: 'copy', items: [], source: 'app', revision: 0 }),
			onClipboard: (listener: (c: Clipboard) => void) => {
				listeners.add(listener);
				return () => listeners.delete(listener);
			},
		} as unknown as OpsClient;
		const service = createClipboardService({
			client,
			vfs: new FakeVfsClient(),
			os: null,
			focusTarget: null,
		});
		await service.ready;
		const set = (revision: number, name: string) =>
			listeners.forEach((listener) =>
				listener({
					mode: 'cut',
					items: [child(FOLDER, name)],
					source: 'app',
					revision,
				}),
			);
		expect(board(service).items).toEqual([]);
		set(5, 'new');
		set(3, 'old');
		expect(board(service).items).toEqual([child(FOLDER, 'new')]);
		expect(board(service).revision).toBe(5);
		service.dispose();
		set(6, 'later');
		expect(board(service).revision).toBe(5);
	});

	it('takes what Rust held before the window existed', async () => {
		const { client, window, dispose } = setup();
		await client.setClipboard('cut', [child(FOLDER, 'held.txt')]);
		const late = window();
		await late.ready;
		expect(board(late)).toMatchObject({ mode: 'cut', items: [child(FOLDER, 'held.txt')] });
		dispose();
	});
});

describe('Copy of locations that are not a listing’s selection', () => {
	it('puts them on the clipboard as a copy and on the system clipboard too', async () => {
		const { client, os, window, dispose } = setup();
		const service = window();
		const locations = [child(FOLDER, 'a.txt'), child(FOLDER, 'z.txt')];
		const set = await service.setFromLocations(locations, 'copy');
		expect(set).toMatchObject({ mode: 'copy', items: locations });
		expect(board(service)).toEqual(set);
		expect(client.calls.find((c) => c[0] === 'setClipboard')).toBeDefined();
		expect(os?.files).toMatchObject({ cut: false });
		dispose();
	});
});

describe('Cut and Copy of a selection', () => {
	it('puts what Rust resolved on the clipboard and says it was the app', async () => {
		const { client, window, dispose } = setup();
		const service = window();
		const set = await service.setFromSelection(1, spec, 'cut');
		expect(set).toMatchObject({ mode: 'cut', source: 'app' });
		expect(set.items).toEqual([child(FOLDER, 'a.txt'), child(FOLDER, 'b.txt')]);
		expect(client.calls.find((c) => c[0] === 'setClipboardFromSelection')).toEqual([
			'setClipboardFromSelection',
			1,
			{ kind: 'some', ids: [1, 2] },
			'cut',
		]);
		expect(board(service)).toEqual(set);
		dispose();
	});

	it('rejects as Rust does for a selection of nothing, leaving the clipboard alone', async () => {
		const client = createFakeOpsClient();
		const service = createClipboardService({
			client,
			vfs: new FakeVfsClient(),
			os: null,
			focusTarget: null,
		});
		await service.ready;
		await expect(service.setFromSelection(1, spec, 'copy')).rejects.toMatchObject({
			kind: 'ops',
		});
		expect(board(service).revision).toBe(0);
		service.dispose();
	});

	it('dims the cut rows of a folder from the mirrored clipboard, and stops when it is cleared or replaced by a copy', async () => {
		const { window, dispose } = setup();
		const service = window();
		await service.setFromSelection(1, spec, 'cut');
		expect([...cutNames(board(service), FOLDER.uri)].sort()).toEqual(['a.txt', 'b.txt']);
		await service.setFromSelection(1, spec, 'copy');
		expect(cutNames(board(service), FOLDER.uri).size).toBe(0);
		await service.setFromSelection(1, spec, 'cut');
		await service.clear();
		expect(board(service).items).toEqual([]);
		expect(cutNames(board(service), FOLDER.uri).size).toBe(0);
		dispose();
	});
});

describe('shared by every window', () => {
	it('shows a Copy in one window to another, and a clear in either to both', async () => {
		const { window, dispose } = setup();
		const first = window();
		const second = window();
		await Promise.all([first.ready, second.ready]);
		await first.setFromSelection(1, spec, 'cut');
		expect(board(second)).toMatchObject({ mode: 'cut', source: 'app' });
		expect(board(second).items).toHaveLength(2);
		expect([...cutNames(board(second), FOLDER.uri)]).toHaveLength(2);
		await second.clear();
		expect(board(first).items).toEqual([]);
		expect(board(first).revision).toBe(board(second).revision);
		dispose();
	});

	it('lets one window paste what another copied, without the system clipboard', async () => {
		const { window, dispose } = setup({ os: null });
		const first = window();
		const second = window();
		await Promise.all([first.ready, second.ready]);
		await first.setFromSelection(1, spec, 'copy');
		const pasted = await second.forPaste();
		expect(pasted.items).toHaveLength(2);
		expect(pasted.source).toBe('app');
		dispose();
	});
});

describe('the system file clipboard', () => {
	it('gets the files from Copy, and from Cut with the cut flag', async () => {
		const { os, window, dispose } = setup();
		const service = window();
		await service.setFromSelection(1, spec, 'copy');
		expect(os!.files).toEqual({
			uris: [child(FOLDER, 'a.txt').uri, child(FOLDER, 'b.txt').uri],
			cut: false,
		});
		await service.setFromSelection(1, spec, 'cut');
		expect(os!.files?.cut).toBe(true);
		dispose();
	});

	it('still copies inside Waypoint when the system clipboard refuses the write', async () => {
		const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
		const { os, window, dispose } = setup({ os: new FakeOsClipboardClient({ refuseSet: true }) });
		const service = window();
		await service.setFromSelection(1, spec, 'copy');
		expect(board(service).items).toHaveLength(2);
		expect(os!.files).toBeNull();
		expect((await service.forPaste()).items).toHaveLength(2);
		warn.mockRestore();
		dispose();
	});

	it('leaves the system clipboard alone where it is unavailable', async () => {
		const { os, window, dispose } = setup({ os: new FakeOsClipboardClient({ available: false }) });
		const service = window();
		await service.setFromSelection(1, spec, 'copy');
		os!.external(OTHER_FILES);
		expect((await service.forPaste()).items).toHaveLength(2);
		expect(os!.calls).toEqual([]);
		dispose();
	});

	it('pastes files another application copied, adopting them as the clipboard from the OS', async () => {
		const { client, os, window, dispose } = setup();
		const service = window();
		await service.ready;
		os!.external(OTHER_FILES);
		const pasted = await service.forPaste();
		expect(pasted).toMatchObject({ mode: 'copy', source: 'os' });
		expect(pasted.items.map((item) => item.uri)).toEqual(OTHER_FILES.uris);
		// Every window sees it: Rust holds it.
		expect((await client.getClipboard()).source).toBe('os');
		dispose();
	});

	it('adopts a cut as a cut, so the paste moves', async () => {
		const { os, window, dispose } = setup();
		const service = window();
		await service.ready;
		os!.external({ ...OTHER_FILES, cut: true });
		expect(await service.syncFromOs()).toBe(true);
		expect(board(service).mode).toBe('cut');
		dispose();
	});

	it('does not adopt the same files twice, nor files that are Waypoint’s own', async () => {
		const { os, window, dispose } = setup();
		const service = window();
		await service.ready;
		os!.external(OTHER_FILES);
		expect(await service.syncFromOs()).toBe(true);
		const revision = board(service).revision;
		expect(await service.syncFromOs()).toBe(false);
		expect(board(service).revision).toBe(revision);
		// Waypoint copies afterwards: its own files are on the system clipboard, and win.
		await service.setFromSelection(1, spec, 'copy');
		const pasted = await service.forPaste();
		expect(pasted.source).toBe('app');
		expect(pasted.items).toHaveLength(2);
		dispose();
	});

	it('prefers files copied elsewhere after Waypoint’s own copy', async () => {
		const { os, window, dispose } = setup();
		const service = window();
		await service.setFromSelection(1, spec, 'copy');
		os!.external(OTHER_FILES);
		const pasted = await service.forPaste();
		expect(pasted.source).toBe('os');
		expect(pasted.items.map((item) => item.uri)).toEqual(OTHER_FILES.uris);
		dispose();
	});

	it('ignores content that is not files, keeping Waypoint’s clipboard', async () => {
		const { os, window, dispose } = setup();
		const service = window();
		await service.setFromSelection(1, spec, 'copy');
		os!.external(null);
		const pasted = await service.forPaste();
		expect(pasted.source).toBe('app');
		expect(pasted.items).toHaveLength(2);
		dispose();
	});

	it('ignores a uri Rust cannot parse', async () => {
		const { os, window, dispose } = setup();
		const service = window();
		await service.ready;
		os!.external({ uris: ['file://'], cut: false });
		vi.spyOn(FakeVfsClient.prototype, 'parseLocation').mockRejectedValue({
			kind: 'invalidLocation',
			input: 'file://',
		});
		expect(await service.syncFromOs()).toBe(false);
		expect(board(service).items).toEqual([]);
		vi.restoreAllMocks();
		dispose();
	});

	it('looks again when the window gains focus and when the system says its clipboard changed', async () => {
		const focus = new EventTarget();
		const { os, window, dispose } = setup({ focus });
		const service = window();
		await service.ready;
		os!.external(OTHER_FILES);
		// The change event is what a real system sends; the fake sends it too.
		await vi.waitFor(() => expect(board(service).source).toBe('os'));
		await service.clear();
		expect(board(service).items).toEqual([]);
		os!.external(null);
		os!.external({ uris: ['file:///home/elsewhere/y.txt'], cut: false });
		await vi.waitFor(() =>
			expect(board(service).items[0]?.uri).toBe('file:///home/elsewhere/y.txt'),
		);
		dispose();
	});

	it('looks on focus alone when the change event does not come', async () => {
		const focus = new EventTarget();
		const os = new FakeOsClipboardClient();
		// A system that never sends the change event.
		os.onChange = () => () => {};
		const { window, dispose } = setup({ os, focus });
		const service = window();
		await service.ready;
		os.external(OTHER_FILES);
		expect(board(service).items).toEqual([]);
		focus.dispatchEvent(new Event('focus'));
		await vi.waitFor(() => expect(board(service).source).toBe('os'));
		dispose();
	});

	it('does not mistake its own Copy for another application’s while writing it', async () => {
		const { os, window, dispose } = setup({ focus: new EventTarget() });
		const service = window();
		await service.ready;
		await service.setFromSelection(1, spec, 'copy');
		await new Promise((resolve) => setTimeout(resolve, 10));
		expect(board(service).source).toBe('app');
		expect(os!.files?.uris).toHaveLength(2);
		dispose();
	});
});
