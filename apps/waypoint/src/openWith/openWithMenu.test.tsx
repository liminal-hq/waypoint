// Verifies the Open With submenu: what is offered by what the plugin reports, and what each row stands for
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Handlers, PluginStatus } from '@liminal-hq/plugin-mime-apps';
import { expectEveryItemHasIcon } from '@liminal-hq/waypoint-chrome/ContextMenu/expectEveryItemHasIcon';
import type { MenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import { describe, expect, it } from 'vitest';
import { createFakeOpenWithClient, fakeApp } from './fakeOpenWithClient';
import { OPEN_WITH_IDS, openWithAction, openWithMenu } from './openWithMenu';

const iconUrl = (id: string) => `appicon://localhost/${id}`;
const handlers = (extra: Partial<Handlers> = {}): Handlers => ({
	mime: 'image/png',
	mixed: false,
	default: fakeApp('viewer.desktop', 'Image Viewer'),
	recommended: [fakeApp('editor.desktop', 'Image Editor')],
	others: [fakeApp('text.desktop', 'Text Editor')],
	...extra,
});
const statusOf = (features?: string[]): Promise<PluginStatus> =>
	createFakeOpenWithClient(features ? { features } : {}).getStatus();
const labels = (items: MenuItem[]) =>
	items.map((item) => (item.type === 'separator' ? '|' : 'label' in item ? item.label : ''));
const build = async (input: { handlers?: Handlers | null; count?: number; features?: string[] }) =>
	openWithMenu({
		status: await statusOf(input.features),
		handlers: input.handlers === undefined ? handlers() : input.handlers,
		count: input.count ?? 1,
		iconUrl,
	});

describe('the Open With submenu', () => {
	it('lists the default, the recommended applications and Other Application…, each with an icon', async () => {
		const menu = await build({});
		expect(menu).toMatchObject({ type: 'submenu', id: 'openWith', label: 'Open With' });
		expect(labels(menu!.items)).toEqual([
			'Image Viewer (default)',
			'|',
			'Image Editor',
			'|',
			'Other Application…',
		]);
		expectEveryItemHasIcon([menu!]);
	});

	it('is not offered when the plugin reports nothing that can open', async () => {
		expect(await build({ features: [] })).toBeNull();
		expect(await build({ features: ['typeInfo', 'appIcons'] })).toBeNull();
	});

	it('is not offered for a selection of more than one type', async () => {
		expect(await build({ handlers: handlers({ mixed: true }) })).toBeNull();
	});

	it('holds one disabled row while the applications are being read', async () => {
		const menu = await build({ handlers: null });
		expect(menu!.items).toEqual([
			expect.objectContaining({ id: OPEN_WITH_IDS.loading, disabled: true }),
		]);
	});

	it('leaves out the default when the type has none, and Other Application… when no other application is installed', async () => {
		const menu = await build({ handlers: handlers({ default: null, others: [] }) });
		expect(labels(menu!.items)).toEqual(['Image Editor']);
		expect(
			await build({ handlers: handlers({ default: null, recommended: [], others: [] }) }),
		).toBeNull();
	});

	it('offers only the default where applications cannot be listed or chosen (a Flatpak)', async () => {
		const menu = await build({ features: ['handlers', 'openDefault'] });
		expect(labels(menu!.items)).toEqual(['Image Viewer (default)']);
	});

	it('uses the system chooser for one file and the dialog for several', async () => {
		const features = ['handlers', 'openWith', 'openDefault', 'chooser'];
		expect(
			(await build({ features, count: 1 }))!.items.some(
				(item) => 'id' in item && item.id === OPEN_WITH_IDS.chooser,
			),
		).toBe(true);
		const several = await build({ features, count: 3 });
		expect(several!.items.some((item) => 'id' in item && item.id === OPEN_WITH_IDS.other)).toBe(
			true,
		);
		expect(several!.items.some((item) => 'id' in item && item.id === OPEN_WITH_IDS.chooser)).toBe(
			false,
		);
	});
});

describe('what a row stands for', () => {
	const list = handlers();

	it('maps the fixed rows and finds an application by its id', () => {
		expect(openWithAction(OPEN_WITH_IDS.default, list)).toEqual({ kind: 'default' });
		expect(openWithAction(OPEN_WITH_IDS.chooser, list)).toEqual({ kind: 'chooser' });
		expect(openWithAction(OPEN_WITH_IDS.other, list)).toEqual({ kind: 'other' });
		expect(openWithAction('openWith:app:editor.desktop', list)).toEqual({
			kind: 'app',
			app: list.recommended[0],
		});
	});

	it('ignores an id that is not the submenu’s, and an application it did not list', () => {
		expect(openWithAction('copyPath', list)).toBeNull();
		expect(openWithAction('openWith:app:other.desktop', list)).toBeNull();
		expect(openWithAction('openWith:app:editor.desktop', null)).toBeNull();
	});
});
