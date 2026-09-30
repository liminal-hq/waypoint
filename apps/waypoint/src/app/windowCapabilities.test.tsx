// Verifies the capabilities provider stays null until known and tolerates a missing plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { WindowCapabilities } from '@liminal-hq/plugin-window-manager';
import { render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const plugin = vi.hoisted(() => ({ read: vi.fn<() => Promise<WindowCapabilities>>() }));
vi.mock('@liminal-hq/plugin-window-manager', () => ({ getCapabilities: plugin.read }));

import { useWindowCapabilities, WindowCapabilitiesProvider } from './windowCapabilities';

function Probe() {
	const capabilities = useWindowCapabilities();
	return (
		<p>
			{capabilities === null ? 'unknown' : `${capabilities.session}:${capabilities.alwaysOnTop}`}
		</p>
	);
}

beforeEach(() => plugin.read.mockReset());

describe('WindowCapabilitiesProvider', () => {
	it('is null first, then the reading', async () => {
		plugin.read.mockResolvedValue({
			session: 'wayland',
			alwaysOnTop: false,
			systemWindowMenu: true,
		});
		render(
			<WindowCapabilitiesProvider>
				<Probe />
			</WindowCapabilitiesProvider>,
		);
		expect(screen.getByText('unknown')).toBeTruthy();
		await waitFor(() => expect(screen.getByText('wayland:false')).toBeTruthy());
	});
});
