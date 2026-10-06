// Verifies a tab's folder icon follows the window's icon theme: the System set's picture once loaded, the Waypoint glyph before it and where the system has none
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { act, cleanup, render } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import { VfsClientProvider } from '../browse/VfsClientContext';
import { configureSystemIcons } from '../icons/systemIcons';
import { createFakeSystemIconsClient } from '../services/fakeSystemIconsClient';
import { createTree } from '../test/workspaceHarness';
import { PaneHeader } from './PaneHeader';

const root = document.documentElement;

const tab = {
	id: 'tab-1',
	location: { display: '/home/scott/Pictures' },
	back: [],
	forward: [],
	pinned: false,
	colour: null,
	group: null,
	hints: {},
} as unknown as TabSnapshot;

const header = () => (
	<VfsClientProvider client={createTree()}>
		<PaneHeader tab={tab} active={false} onClose={() => {}} />
	</VfsClientProvider>
);

async function settle(): Promise<void> {
	await act(async () => {
		for (let turn = 0; turn < 5; turn += 1) await Promise.resolve();
	});
}

beforeEach(() => {
	root.dataset.iconTheme = 'system';
});

afterEach(() => {
	cleanup();
	configureSystemIcons(null);
	delete root.dataset.iconTheme;
});

describe('a tab’s folder icon', () => {
	it('draws the Waypoint folder glyph until the system’s folder icon has loaded, then the picture', async () => {
		const fake = createFakeSystemIconsClient();
		configureSystemIcons(fake);
		const { container } = render(header());
		expect(container.querySelector('svg[data-group="folder"]:not([data-system])')).not.toBeNull();
		await settle();
		await act(async () => fake.settle(true));
		expect(container.querySelector('svg[data-system] image')).not.toBeNull();
		expect(container.querySelector('svg[data-group="folder"]:not([data-system])')).toBeNull();
	});

	it('keeps the Waypoint glyph where the system has no folder icons', async () => {
		const fake = createFakeSystemIconsClient({
			status: { folderIcons: { available: false, reason: 'no theme for folders' } },
		});
		configureSystemIcons(fake);
		const { container } = render(header());
		await settle();
		expect(container.querySelector('svg[data-system]')).toBeNull();
		expect(container.querySelector('svg[data-group="folder"]')).not.toBeNull();
	});
});

describe('the tab switcher’s candidate row', () => {
	it('draws its folder icon in the contrast colour, as the accent icon would vanish on the accent fill', () => {
		const css = readFileSync(join(import.meta.dirname, 'TabSwitcher.module.css'), 'utf8');
		expect(css).toMatch(
			/\.row\[data-candidate\] \.icon \{[^}]*color: var\(--wp-accent-contrast\);[^}]*\}/,
		);
		expect(css).toMatch(/\.row\[data-candidate\] \.icon \[data-fill\] \{[^}]*fill: /);
	});
});
