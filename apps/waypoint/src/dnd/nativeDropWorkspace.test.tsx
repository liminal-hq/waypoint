// Verifies native drops and outbound drags in the whole browsing area, over the fake plugin: files onto a row, a place, a tab and the + button, a row out of the window and back
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { dismissNotice } from '../app/notices';
import { createFakeOpsClient, type FakeOpsClient } from '../services/fakeOpsClient';
import { FakeNativeDndClient } from '../services/fakeNativeDndClient';
import { FakeTearoffClient } from '../services/fakeTearoffClient';
import { FakePlacesClient, fakePlaces } from '../services/fakePlacesClient';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { createFakeSettingsClient } from '../services/fakeSettingsClient';
import { DEFAULT_SETTINGS } from '../services/settingsClient';
import { stubLayout } from '../test/browseHarness';
import { createTree, DOCS, HOME, renderWorkspace } from '../test/workspaceHarness';
import { PLAN_REST_MS } from './fileDrag';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
	localStorage.clear();
	document.elementsFromPoint = () => [];
});
afterEach(() => {
	cleanup();
	dismissNotice();
	restoreLayout();
	// @ts-expect-error happy-dom has no hit test; the tests stand one in
	delete document.elementsFromPoint;
});

async function mount(
	options: {
		native?: FakeNativeDndClient;
		sidebar?: boolean;
		rule?: 'byVolume' | 'alwaysCopy' | 'alwaysAsk';
		tearoff?: FakeTearoffClient;
	} = {},
) {
	const vfs = createTree();
	const tabs = new FakeTabsApi();
	await tabs.openTab(HOME);
	const ops: FakeOpsClient = createFakeOpsClient({
		resolveSelection: async (handle, spec) =>
			Promise.all(spec.ids.map((id) => vfs.entryLocation(handle, id))),
	});
	const native = options.native ?? new FakeNativeDndClient();
	await renderWorkspace(vfs, tabs, new FakePlacesClient({ places: fakePlaces('/home/test') }), {
		sidebar: options.sidebar ?? false,
		ops,
		nativeDnd: native,
		...(options.rule
			? {
					settings: createFakeSettingsClient({
						...DEFAULT_SETTINGS,
						dnd: { ...DEFAULT_SETTINGS.dnd, defaultActionRule: options.rule },
					}),
				}
			: {}),
	});
	// The plugin's status has been asked for, so its events are listened to.
	await waitFor(() => expect(native.listenerCount()).toBeGreaterThan(0));
	return { ops, tabs, native };
}

const options = () =>
	within(screen.getAllByRole('listbox', { name: /^Files/ })[0]!).getAllByRole('option');
const row = (name: string) => options().find((option) => option.textContent?.includes(name))!;
const submits = (ops: FakeOpsClient) => ops.calls.filter((call) => call[0] === 'submit');
const lastSubmit = (ops: FakeOpsClient) => submits(ops).at(-1)![1] as Record<string, unknown>;
const pointAt = (...elements: Element[]) => {
	document.elementsFromPoint = () => elements;
};
const pill = () => document.querySelector('[data-drag-pill]')?.textContent ?? null;
const rest = () =>
	act(async () => void (await new Promise((done) => setTimeout(done, PLAN_REST_MS + 60))));

const FILES = ['file:///srv/share/with%20space.txt', 'file:///srv/share/bad%FF%FE.txt'];

describe('files dragged in from another application', () => {
	it('copy onto a folder row as a job over the lossless URIs, and move with Shift', async () => {
		const { ops, native } = await mount();
		await waitFor(() => expect(row('docs')).toBeDefined());
		native.enter(FILES, { x: 100, y: 100 });
		pointAt(row('docs'));
		native.over({ x: 140, y: 100 });
		await waitFor(() => expect(pill()).toContain('Copy 2 items to docs'));
		await rest();
		await waitFor(() => expect(pill()).toContain('Copy 2 items to docs'));
		expect(row('docs')).toHaveAttribute('data-drop-over', 'ok');
		native.drop(FILES, { x: 140, y: 100 });
		await waitFor(() => expect(submits(ops)).toHaveLength(1));
		expect(lastSubmit(ops)).toMatchObject({
			kind: { kind: 'copy' },
			sources: {
				kind: 'locations',
				locations: [
					expect.objectContaining({ uri: FILES[0] }),
					expect.objectContaining({ uri: FILES[1] }),
				],
			},
			destination: { uri: DOCS.uri },
			originWindow: 'main-1',
		});
		expect(row('docs')).not.toHaveAttribute('data-drop-over');
		expect(pill()).toBeNull();
	});

	it('moves with Shift where the platform reports it', async () => {
		const { ops, native } = await mount();
		await waitFor(() => expect(row('docs')).toBeDefined());
		native.enter(FILES, { x: 100, y: 100 });
		pointAt(row('docs'));
		native.over({ x: 140, y: 100 }, { modifiers: { shift: true } });
		await waitFor(() => expect(pill()).toContain('Move 2 items to docs'));
		native.drop(FILES, { x: 140, y: 100 }, { modifiers: { shift: true } });
		await waitFor(() => expect(submits(ops)).toHaveLength(1));
		expect(lastSubmit(ops)).toMatchObject({ kind: { kind: 'move' } });
	});

	it('copies with Ctrl where the platform reports it, and the queue says the job is under way', async () => {
		const { ops, native } = await mount();
		await waitFor(() => expect(row('docs')).toBeDefined());
		native.enter(FILES, { x: 100, y: 100 });
		pointAt(row('docs'));
		native.over({ x: 140, y: 100 }, { modifiers: { ctrl: true } });
		await waitFor(() => expect(pill()).toContain('Copy 2 items to docs'));
		native.drop(FILES, { x: 140, y: 100 }, { modifiers: { ctrl: true } });
		await waitFor(() => expect(submits(ops)).toHaveLength(1));
		expect(lastSubmit(ops)).toMatchObject({ kind: { kind: 'copy' } });
	});

	it('onto a sidebar place', async () => {
		const { ops, native } = await mount({ sidebar: true });
		const places = within(screen.getByRole('navigation', { name: 'Sidebar' })).getByRole('group', {
			name: 'Places',
		});
		const documents = await within(places).findByRole('button', { name: /^Documents/ });
		native.enter(FILES, { x: 100, y: 100 });
		pointAt(documents);
		native.over({ x: 20, y: 100 });
		await rest();
		native.drop(FILES, { x: 20, y: 100 }, { modifiers: { ctrl: true } });
		await waitFor(() => expect(submits(ops)).toHaveLength(1));
		expect(lastSubmit(ops)).toMatchObject({
			kind: { kind: 'copy' },
			destination: { uri: 'file:///home/test/Documents' },
		});
	});

	it('onto another tab, and the + button opens the dropped folder', async () => {
		const { ops, tabs, native } = await mount();
		await tabs.openTab(DOCS, { activate: false });
		const tab = await waitFor(() => {
			const found = document.querySelector('[data-drop="tab"][data-drop-label="docs"]');
			expect(found).not.toBeNull();
			return found!;
		});
		native.enter(FILES, { x: 100, y: 100 });
		pointAt(tab);
		native.over({ x: 140, y: 100 }, { modifiers: { ctrl: true } });
		await rest();
		native.drop(FILES, { x: 140, y: 100 }, { modifiers: { ctrl: true } });
		await waitFor(() => expect(submits(ops)).toHaveLength(1));
		expect(lastSubmit(ops)).toMatchObject({ destination: { uri: DOCS.uri } });

		native.enter(['file:///home/test/docs'], { x: 100, y: 100 });
		pointAt(screen.getByRole('button', { name: 'New tab' }));
		native.over({ x: 140, y: 100 });
		await waitFor(() => expect(pill()).toContain('Open in a new tab'));
		native.drop(['file:///home/test/docs'], { x: 140, y: 100 });
		await waitFor(async () => expect((await tabs.getSnapshot()).tabs).toHaveLength(3));
		expect(submits(ops)).toHaveLength(1);
	});

	it('asks at the drop when the setting is to ask, as it must where the keys are unknown (Wayland)', async () => {
		const native = new FakeNativeDndClient({
			availability: { modifiers: false, displayServer: 'wayland' },
		});
		const { ops } = await mount({ native, rule: 'alwaysAsk' });
		await waitFor(() => expect(row('docs')).toBeDefined());
		native.enter(FILES, { x: 100, y: 100 });
		pointAt(row('docs'));
		native.over({ x: 140, y: 100 });
		native.drop(FILES, { x: 140, y: 100 });
		const menu = await screen.findByRole('menu', { name: 'Drop action' });
		fireEvent.click(within(menu).getByRole('menuitem', { name: 'Copy Here' }));
		await waitFor(() => expect(submits(ops)).toHaveLength(1));
		expect(lastSubmit(ops)).toMatchObject({ kind: { kind: 'copy' } });
	});

	it('clears the highlight when the files leave the window, and runs nothing', async () => {
		const { ops, native } = await mount();
		await waitFor(() => expect(row('docs')).toBeDefined());
		native.enter(FILES, { x: 100, y: 100 });
		pointAt(row('docs'));
		native.over({ x: 140, y: 100 });
		await waitFor(() => expect(row('docs')).toHaveAttribute('data-drop-over'));
		native.leave();
		expect(row('docs')).not.toHaveAttribute('data-drop-over');
		await waitFor(() => expect(pill()).toBeNull());
		expect(submits(ops)).toHaveLength(0);
	});

	it('leaves a drag of text or a link alone', async () => {
		const { ops, native } = await mount();
		native.enter(['https://example.com/'], { x: 100, y: 100 });
		expect(pill()).toBeNull();
		native.drop(['https://example.com/'], { x: 100, y: 100 });
		expect(submits(ops)).toHaveLength(0);
	});
});

describe('a row dragged out of the window', () => {
	async function grab(name: string) {
		await waitFor(() => expect(row(name)).toBeDefined());
		fireEvent.pointerDown(row(name), { pointerId: 1, button: 0, clientX: 100, clientY: 100 });
		fireEvent.pointerMove(window, { pointerId: 1, clientX: 120, clientY: 100 });
	}

	it('becomes the system’s drag of the selection’s URIs, and the page is free again', async () => {
		const { ops, native } = await mount();
		await grab('notes.txt');
		expect(pill()).toContain('Dragging notes.txt');
		fireEvent.pointerMove(window, { pointerId: 1, clientX: -8, clientY: 100 });
		await waitFor(() => expect(native.started).toHaveLength(1));
		expect(native.started[0]).toMatchObject({
			uris: ['file:///home/test/notes.txt'],
			actions: ['copy', 'move', 'link'],
		});
		expect(ops.calls.filter((call) => call[0] === 'resolveSelection')).toHaveLength(1);
		await waitFor(() => expect(pill()).toBeNull());
		// The synthetic release the plugin sends arrives as a pointer-up with no drag behind it.
		fireEvent.pointerUp(window, { pointerId: 1, clientX: 5, clientY: 100 });
		// A second drag works.
		native.endDrag('dropped-copy');
		await grab('photo.jpg');
		expect(pill()).toContain('Dragging photo.jpg');
		fireEvent.keyDown(window, { key: 'Escape' });
	});

	it('runs no job here when the other application moved the files', async () => {
		const { ops, native } = await mount();
		await grab('notes.txt');
		fireEvent.pointerMove(window, { pointerId: 1, clientX: 2000, clientY: 100 });
		await waitFor(() => expect(native.started).toHaveLength(1));
		native.endDrag('dropped-move');
		expect(submits(ops)).toHaveLength(0);
		expect(ops.calls.filter((call) => call[0] === 'plan')).toHaveLength(0);
	});

	it('stays in the page when the plugin has no outbound drag, and leaving does nothing', async () => {
		const native = new FakeNativeDndClient({ availability: { outbound: false } });
		const { ops } = await mount({ native });
		await grab('notes.txt');
		fireEvent.pointerMove(window, { pointerId: 1, clientX: -8, clientY: 100 });
		await new Promise((done) => setTimeout(done, 20));
		expect(native.started).toHaveLength(0);
		expect(ops.calls.filter((call) => call[0] === 'resolveSelection')).toHaveLength(0);
		expect(pill()).toContain('Dragging notes.txt');
		fireEvent.keyDown(window, { key: 'Escape' });
	});

	it('stays in the page with a notice when the system refuses', async () => {
		const native = new FakeNativeDndClient();
		native.refuseStart = 'buttonNotPressed';
		await mount({ native });
		await grab('notes.txt');
		fireEvent.pointerMove(window, { pointerId: 1, clientX: -8, clientY: 100 });
		await screen.findByText(/stays here/);
		expect(pill()).not.toBeNull();
		fireEvent.keyDown(window, { key: 'Escape' });
	});

	it('is the same as a drop from another application when it comes back to this window', async () => {
		const { ops, native } = await mount();
		await grab('notes.txt');
		fireEvent.pointerMove(window, { pointerId: 1, clientX: -8, clientY: 100 });
		await waitFor(() => expect(native.started).toHaveLength(1));
		const uris = native.started[0]!.uris;
		native.enter(uris, { x: 100, y: 100 });
		pointAt(row('docs'));
		native.over({ x: 140, y: 100 }, { modifiers: { shift: true } });
		await waitFor(() => expect(pill()).toContain('Move notes.txt to docs'));
		native.drop(uris, { x: 140, y: 100 }, { modifiers: { shift: true }, selfDrop: true });
		await waitFor(() => expect(submits(ops)).toHaveLength(1));
		expect(lastSubmit(ops)).toMatchObject({
			kind: { kind: 'move' },
			sources: { kind: 'locations' },
			destination: { uri: DOCS.uri },
		});
	});
});

describe('tab drags', () => {
	it('never start a tear-off from a file drag leaving the window', async () => {
		const tearoff = new FakeTearoffClient();
		const { native } = await mount({ tearoff });
		await waitFor(() => expect(row('notes.txt')).toBeDefined());
		fireEvent.pointerDown(row('notes.txt'), {
			pointerId: 1,
			button: 0,
			clientX: 100,
			clientY: 100,
		});
		fireEvent.pointerMove(window, { pointerId: 1, clientX: 120, clientY: 100 });
		fireEvent.pointerMove(window, { pointerId: 1, clientX: -8, clientY: 100 });
		await waitFor(() => expect(native.started).toHaveLength(1));
		expect(tearoff.calls.filter((call) => call === 'begin' || call.startsWith('end'))).toEqual([]);
	});
});
