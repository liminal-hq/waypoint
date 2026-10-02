// Verifies the click setting: a single click opens in single-click mode, only a double click does otherwise
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createFakeSettingsClient } from '../services/fakeSettingsClient';
import { DEFAULT_SETTINGS, type ClickMode } from '../services/settingsClient';
import type { VfsClient } from '../services/vfsClient';
import { SettingsProvider } from '../settings/SettingsContext';
import { clientWith, FOLDER, stubLayout } from '../test/browseHarness';
import { GridView } from './GridView';
import { ListView } from './ListView';
import { useListingSession } from './useListingSession';
import { useVfsClient, VfsClientProvider } from './VfsClientContext';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
});
afterEach(() => {
	cleanup();
	restoreLayout();
});

type View = (props: { onOpen: (entry: Entry) => void }) => React.JSX.Element;

const List: View = ({ onOpen }) => <ListView location={FOLDER} onOpen={onOpen} />;

const Grid: View = ({ onOpen }) => {
	const state = useListingSession(useVfsClient(), FOLDER);
	return <GridView state={state} size={96} onOpen={onOpen} />;
};

async function show(
	View: View,
	clickMode: ClickMode | null,
	client: VfsClient,
	onOpen: (entry: Entry) => void,
) {
	const settings = createFakeSettingsClient({
		...DEFAULT_SETTINGS,
		general: { ...DEFAULT_SETTINGS.general, clickMode: clickMode ?? 'double' },
	});
	const view = <View onOpen={onOpen} />;
	render(
		<VfsClientProvider client={client}>
			{clickMode === null ? view : <SettingsProvider client={settings}>{view}</SettingsProvider>}
		</VfsClientProvider>,
	);
	await screen.findByRole('listbox');
	await waitFor(() => expect(screen.queryAllByRole('option').length).toBeGreaterThan(3));
	// The provider reads the settings after the first render.
	await act(async () => {
		await settings.snapshot();
	});
	return screen.getAllByRole('option');
}

describe.each([
	['the list', List],
	['the grid', Grid],
])('%s', (_name, View) => {
	it('opens an entry on a single click when the settings say so, and not again on a double click', async () => {
		const onOpen = vi.fn();
		const rows = await show(View, 'single', clientWith(100).client, onOpen);
		fireEvent.click(rows[2]!);
		expect(onOpen).toHaveBeenCalledTimes(1);
		expect(rows[2]).toHaveAttribute('aria-selected', 'true');
		fireEvent.doubleClick(rows[2]!);
		expect(onOpen).toHaveBeenCalledTimes(1);
	});

	it('keeps Ctrl, Shift and a click in a field out of the opening', async () => {
		const onOpen = vi.fn();
		const rows = await show(View, 'single', clientWith(100).client, onOpen);
		fireEvent.click(rows[1]!, { ctrlKey: true });
		fireEvent.click(rows[3]!, { shiftKey: true });
		expect(onOpen).not.toHaveBeenCalled();
		const field = document.createElement('input');
		rows[0]!.append(field);
		fireEvent.click(field);
		expect(onOpen).not.toHaveBeenCalled();
	});

	it('opens only on a double click by default, and without any settings', async () => {
		for (const mode of ['double', null] as const) {
			const onOpen = vi.fn();
			const rows = await show(View, mode, clientWith(100).client, onOpen);
			fireEvent.click(rows[2]!);
			expect(onOpen).not.toHaveBeenCalled();
			fireEvent.doubleClick(rows[2]!);
			expect(onOpen).toHaveBeenCalledTimes(1);
			cleanup();
		}
	});
});
