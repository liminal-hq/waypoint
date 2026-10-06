// Guards that every virtualiser leaves `useFlushSync` off, so React does not log flushSync errors from a render
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import * as virtual from '@tanstack/react-virtual';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { clientWith, FOLDER, stubLayout } from '../test/browseHarness';
import { GridView } from './GridView';
import { ListView } from './ListView';
import { useListingSession } from './useListingSession';
import { VfsClientProvider, useVfsClient } from './VfsClientContext';

vi.mock('@tanstack/react-virtual', async (importOriginal) => {
	const actual = await importOriginal<typeof import('@tanstack/react-virtual')>();
	return { ...actual, useVirtualizer: vi.fn(actual.useVirtualizer) };
});

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(400, 600);
});
afterEach(() => {
	cleanup();
	restoreLayout();
	vi.mocked(virtual.useVirtualizer).mockClear();
});

function Grid() {
	const state = useListingSession(useVfsClient(), FOLDER);
	return <GridView state={state} size={96} />;
}

function optionsUsed() {
	const calls = vi.mocked(virtual.useVirtualizer).mock.calls;
	expect(calls.length).toBeGreaterThan(0);
	return calls.map(([options]) => options);
}

describe('the virtualisers', () => {
	it('keep useFlushSync off in the list', async () => {
		const { client } = clientWith(50);
		render(
			<VfsClientProvider client={client}>
				<ListView location={FOLDER} />
			</VfsClientProvider>,
		);
		await waitFor(() => expect(screen.queryAllByRole('option').length).toBeGreaterThan(0));
		for (const options of optionsUsed()) {
			expect(options.useFlushSync).toBe(false);
			// A scroll step is drawn by the view itself (`useScrollStepRender`).
			expect(options.onChange).toBeTypeOf('function');
		}
	});

	it('keep useFlushSync off in the grid', async () => {
		const { client } = clientWith(50);
		render(
			<VfsClientProvider client={client}>
				<Grid />
			</VfsClientProvider>,
		);
		await waitFor(() => expect(screen.queryAllByRole('option').length).toBeGreaterThan(0));
		for (const options of optionsUsed()) {
			expect(options.useFlushSync).toBe(false);
			// A scroll step is drawn by the view itself (`useScrollStepRender`).
			expect(options.onChange).toBeTypeOf('function');
		}
	});
});
