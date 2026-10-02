// Tests for the real BatchRenameApi with the operations plugin's guest-js module mocked
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { buildRequest } from './batchRenameModel';
import { createTauriBatchRenameApi } from './tauriBatchRenameApi';

const plugin = vi.hoisted(() => ({
	previewBatchRename: vi.fn(),
	submit: vi.fn(),
}));
vi.mock('@liminal-hq/waypoint-plugin-ops', () => plugin);

const request = buildRequest({ kind: 'locations', locations: [] }, [], 0);

beforeEach(() => {
	for (const fn of Object.values(plugin)) fn.mockReset();
});

describe('createTauriBatchRenameApi', () => {
	it('asks the plugin for the preview of the request as it is', async () => {
		const preview = { rows: [], ruleErrors: [], nowMs: 1, problems: 0, changes: 0 };
		plugin.previewBatchRename.mockResolvedValue(preview);
		expect(await createTauriBatchRenameApi().preview(request)).toBe(preview);
		expect(plugin.previewBatchRename).toHaveBeenCalledWith(request);
	});

	it('submits the request and returns the job', async () => {
		plugin.submit.mockResolvedValue(12);
		expect(await createTauriBatchRenameApi().apply(request)).toBe(12);
		expect(plugin.submit).toHaveBeenCalledWith(request);
	});

	it('lets a rejection through for the dialog to word', async () => {
		const failure = { kind: 'ops', message: 'gone' };
		plugin.previewBatchRename.mockRejectedValue(failure);
		await expect(createTauriBatchRenameApi().preview(request)).rejects.toBe(failure);
	});
});
