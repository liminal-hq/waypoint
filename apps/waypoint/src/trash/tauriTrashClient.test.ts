// Tests for the real TrashClient with the plugins' guest-js modules mocked
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { JobRequest } from '@liminal-hq/waypoint-protocol/generated/JobRequest';
import type { OpsEvent } from '@liminal-hq/waypoint-protocol/generated/OpsEvent';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createTauriTrashClient } from './tauriTrashClient';

const vfs = vi.hoisted(() => ({ getTrashInfo: vi.fn() }));
const ops = vi.hoisted(() => ({
	submit: vi.fn(),
	resolve: vi.fn(),
	resolveError: vi.fn(),
	cancel: vi.fn(),
	onOpsEvent: vi.fn(),
}));
vi.mock('@liminal-hq/waypoint-plugin-vfs', () => vfs);
vi.mock('@liminal-hq/waypoint-plugin-ops', () => ops);

beforeEach(() => {
	for (const fn of [...Object.values(vfs), ...Object.values(ops)]) fn.mockReset();
});

const request: JobRequest = {
	kind: { kind: 'emptyTrash', olderThanDays: null },
	sources: { kind: 'locations', locations: [] },
	destination: null,
	name: null,
	options: { conflict: null, verify: null },
	originWindow: '',
};

describe('createTauriTrashClient', () => {
	it('reads the Trash from the file system plugin', async () => {
		const info = { available: true, reason: null, count: 4 };
		vfs.getTrashInfo.mockResolvedValue(info);
		expect(await createTauriTrashClient().getInfo()).toBe(info);
	});

	it('hands jobs and answers to the operations plugin', async () => {
		const client = createTauriTrashClient();
		ops.submit.mockResolvedValue(12);
		expect(await client.submit(request)).toBe(12);
		expect(ops.submit).toHaveBeenCalledWith(request);
		await client.resolveConflicts(12, 'keepBoth');
		// One policy for every clash the job has: no per-source answers.
		expect(ops.resolve).toHaveBeenCalledWith(12, [], 'keepBoth');
		await client.resolveError(12, 'createParents');
		expect(ops.resolveError).toHaveBeenCalledWith(12, 'createParents');
		await client.cancel(12);
		expect(ops.cancel).toHaveBeenCalledWith(12);
	});

	it('listens to the queue until told to stop, even if it stops before the listener is registered', async () => {
		const event: OpsEvent = { kind: 'jobRemoved', id: 1, revision: 2 };
		let deliver: (event: OpsEvent) => void = () => {};
		const unlisten = vi.fn();
		ops.onOpsEvent.mockImplementation((listener: (event: OpsEvent) => void) => {
			deliver = listener;
			return Promise.resolve(unlisten);
		});
		const client = createTauriTrashClient();
		const heard = vi.fn();
		const stop = client.onEvent(heard);
		await Promise.resolve();
		deliver(event);
		expect(heard).toHaveBeenCalledWith(event);
		stop();
		expect(unlisten).toHaveBeenCalledOnce();
		deliver(event);
		expect(heard).toHaveBeenCalledOnce();

		// Stopped at once: the late registration is undone.
		const late = vi.fn();
		ops.onOpsEvent.mockImplementation(() => Promise.resolve(late));
		createTauriTrashClient().onEvent(() => {})();
		await Promise.resolve();
		await Promise.resolve();
		expect(late).toHaveBeenCalledOnce();
	});
});
