// Verifies the real IntegrationsClient asks the app's own commands
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createTauriIntegrationsClient } from './tauriIntegrationsClient';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));

beforeEach(() => {
	invoke.mockReset();
});

describe('createTauriIntegrationsClient', () => {
	it('reads availability and statuses through the app commands', async () => {
		const client = createTauriIntegrationsClient();
		invoke.mockResolvedValueOnce({ notifications: { available: true, reason: null } });
		expect(await client.availability()).toEqual({
			notifications: { available: true, reason: null },
		});
		expect(invoke).toHaveBeenLastCalledWith('get_integration_availability');
		invoke.mockResolvedValueOnce({ 'xdg-portal': { available: false, reason: 'x', features: [] } });
		expect(await client.statuses()).toEqual({
			'xdg-portal': { available: false, reason: 'x', features: [] },
		});
		expect(invoke).toHaveBeenLastCalledWith('get_integration_statuses');
	});

	it('rejects when the command does', async () => {
		invoke.mockRejectedValue(new Error('not allowed'));
		await expect(createTauriIntegrationsClient().availability()).rejects.toThrow('not allowed');
	});
});
