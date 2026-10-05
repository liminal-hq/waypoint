// Verifies the Storage class column: hidden by default and only on S3, shown from the header menu, words (and a restore note) in every cell
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { createFakeSettingsClient } from '../services/fakeSettingsClient';
import { FakeVfsClient, makeEntry } from '../services/fakeVfsClient';
import { DEFAULT_SETTINGS, type Settings } from '../services/settingsClient';
import { SettingsProvider } from '../settings/SettingsContext';
import { FOLDER, stubLayout } from '../test/browseHarness';
import { ListView } from './ListView';
import { hasStorageClasses, storageClassLabel, storageClassNeedsRestore } from './storageClass';
import { VfsClientProvider } from './VfsClientContext';

const BUCKET: Location = {
	display: 's3://photos (minio.lan:9000)',
	uri: 's3://photos/?endpoint=http%3A%2F%2Fminio.lan%3A9000',
};

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(400, 900);
});
afterEach(() => {
	cleanup();
	restoreLayout();
});

const withClass = (id: number, name: string, storageClass: string): Entry =>
	makeEntry(id, name, { attributes: { 's3.storageClass': storageClass } });

const entries = (): Entry[] => [
	makeEntry(1, '2026', { kind: 'directory' }),
	withClass(2, 'standard.txt', 'STANDARD'),
	withClass(3, 'cold.bin', 'GLACIER'),
	withClass(4, 'deep.bin', 'DEEP_ARCHIVE'),
	withClass(5, 'odd.bin', 'COLD_TIER'),
	makeEntry(6, 'plain.txt'),
];

function mount(location: Location, ui: Partial<Settings['ui']> = {}) {
	const vfs = new FakeVfsClient();
	vfs.setFolder(location, entries());
	const settings = createFakeSettingsClient({
		...DEFAULT_SETTINGS,
		ui: { ...DEFAULT_SETTINGS.ui, ...ui },
	});
	render(
		<SettingsProvider client={settings}>
			<VfsClientProvider client={vfs}>
				<ListView location={location} />
			</VfsClientProvider>
		</SettingsProvider>,
	);
	return { settings };
}

const row = (name: string) =>
	screen.getAllByRole('option').find((option) => within(option).queryByText(name) !== null)!;
const cell = (name: string) =>
	row(name).querySelector('[data-column="storageClass"]') as HTMLElement;

describe('the Storage class column', () => {
	it('is hidden by default, even on S3, and the header menu offers it', async () => {
		const { settings } = mount(BUCKET);
		await waitFor(() => expect(screen.getAllByRole('option')).toHaveLength(6));
		expect(screen.queryByText('Storage class')).toBeNull();
		fireEvent.contextMenu(screen.getByRole('group', { name: 'Sort the list' }));
		const item = await screen.findByRole('menuitemcheckbox', { name: 'Show Storage Class' });
		expect(item).toHaveAttribute('aria-checked', 'false');
		await userEvent.click(item);
		await waitFor(() => expect(settings.uiCalls.at(-1)).toEqual({ storageClassColumn: true }));
		await screen.findByText('Storage class');
	});

	it('shows each object’s class in words, a note for an archived one, and a dash for a folder', async () => {
		mount(BUCKET, { storageClassColumn: true });
		await screen.findByText('Storage class');
		await waitFor(() => expect(screen.getAllByRole('option')).toHaveLength(6));
		expect(cell('standard.txt')).toHaveTextContent('Standard');
		expect(cell('standard.txt')).not.toHaveAttribute('data-archived');
		expect(cell('cold.bin')).toHaveTextContent('Glacier Flexible Retrieval');
		expect(cell('cold.bin')).toHaveTextContent('archived: needs a restore before it can be read');
		expect(cell('cold.bin')).toHaveAttribute('data-archived');
		expect(cell('deep.bin')).toHaveTextContent('Glacier Deep Archive');
		// A class the catalogue does not know shows as the service spelled it.
		expect(cell('odd.bin')).toHaveTextContent('COLD_TIER');
		expect(cell('2026')).toHaveTextContent('—');
		expect(cell('plain.txt')).toHaveTextContent('—');
	});

	it('is not offered outside S3, whatever the setting says', async () => {
		mount(FOLDER, { storageClassColumn: true });
		await waitFor(() => expect(screen.getAllByRole('option').length).toBeGreaterThan(0));
		expect(screen.queryByText('Storage class')).toBeNull();
		fireEvent.contextMenu(screen.getByRole('group', { name: 'Sort the list' }));
		expect(screen.queryByRole('menuitemcheckbox', { name: 'Show Storage Class' })).toBeNull();
	});
});

describe('storage class words', () => {
	it('names the known classes and passes others through', () => {
		expect(storageClassLabel('STANDARD_IA')).toBe('Standard-IA');
		expect(storageClassLabel('GLACIER_IR')).toBe('Glacier Instant Retrieval');
		expect(storageClassLabel('SOMETHING_NEW')).toBe('SOMETHING_NEW');
		expect(storageClassNeedsRestore('GLACIER')).toBe(true);
		expect(storageClassNeedsRestore('GLACIER_IR')).toBe(false);
		expect(hasStorageClasses('s3://b/?endpoint=x')).toBe(true);
		expect(hasStorageClasses('file:///home')).toBe(false);
	});
});
