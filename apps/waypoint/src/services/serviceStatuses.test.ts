// Verifies the plugins' own statuses become the shared shape the Services panel reads
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
	collectServiceStatuses,
	nativeDndServiceStatus,
	SERVICE_SOURCES,
	trashServiceStatus,
} from './serviceStatuses';

const plugins = vi.hoisted(() => ({
	trash: vi.fn(),
	dnd: vi.fn(),
	ops: vi.fn(),
	vfs: vi.fn(),
	windowManager: vi.fn(),
}));
vi.mock('@liminal-hq/plugin-trash', () => ({ getStatus: plugins.trash }));
vi.mock('@liminal-hq/plugin-native-dnd', () => ({ getStatus: plugins.dnd }));
vi.mock('@liminal-hq/waypoint-plugin-ops', () => ({ getStatus: plugins.ops }));
vi.mock('@liminal-hq/waypoint-plugin-vfs', () => ({ getStatus: plugins.vfs }));
vi.mock('@liminal-hq/plugin-window-manager', () => ({ getStatus: plugins.windowManager }));

beforeEach(() => {
	for (const fn of Object.values(plugins)) fn.mockReset();
});

const feature = (available: boolean, reason: string | null = null) => ({ available, reason });

describe('the Trash status', () => {
	it('lists the features that work and has no reason when everything does', async () => {
		plugins.trash.mockResolvedValue({
			available: true,
			reason: null,
			flavour: 'freedesktop',
			features: [
				{ name: 'trash', ...feature(true) },
				{ name: 'list', ...feature(true) },
				{ name: 'restore', ...feature(true) },
			],
		});
		expect(await trashServiceStatus()).toEqual({
			available: true,
			reason: null,
			features: ['trash', 'list', 'restore'],
		});
	});

	it('stays available inside a sandbox, without the features it lacks, and says why', async () => {
		const only = 'the Trash portal can only move files to the trash';
		plugins.trash.mockResolvedValue({
			available: true,
			reason: null,
			flavour: 'portal',
			features: [
				{ name: 'trash', ...feature(true) },
				{ name: 'list', ...feature(false, only) },
				{ name: 'restore', ...feature(false, only) },
				{ name: 'empty', ...feature(false, only) },
			],
		});
		expect(await trashServiceStatus()).toEqual({
			available: true,
			reason: only,
			features: ['trash'],
		});
	});

	it('is unavailable with the plugin’s own reason where nothing works', async () => {
		plugins.trash.mockResolvedValue({
			available: false,
			reason: 'no trash on this system',
			flavour: 'unsupported',
			features: [{ name: 'trash', ...feature(false, 'no trash on this system') }],
		});
		expect(await trashServiceStatus()).toEqual({
			available: false,
			reason: 'no trash on this system',
			features: [],
		});
	});
});

describe('the native drag and drop status', () => {
	it('reads its feature map', async () => {
		plugins.dnd.mockResolvedValue({
			available: true,
			reason: null,
			displayServer: 'wayland',
			features: {
				'outbound-drag': feature(true),
				clipboard: feature(false, 'no clipboard portal'),
			},
		});
		expect(await nativeDndServiceStatus()).toEqual({
			available: true,
			reason: 'no clipboard portal',
			features: ['outbound-drag'],
		});
	});
});

describe('the Services panel sources', () => {
	it('reports trash, native-dnd and waypoint-ops, and one that cannot answer does not hide the others', async () => {
		expect(Object.keys(SERVICE_SOURCES)).toEqual([
			'file-system',
			'trash',
			'native-dnd',
			'window-manager',
			'waypoint-ops',
		]);
		plugins.vfs.mockResolvedValue({ available: true, reason: null, features: ['listing'] });
		plugins.trash.mockRejectedValue(new Error('permission denied'));
		plugins.dnd.mockResolvedValue({
			available: false,
			reason: 'no drag and drop here',
			displayServer: 'x11',
			features: {},
		});
		plugins.ops.mockResolvedValue({ available: true, reason: null, features: ['queue', 'trash'] });
		plugins.windowManager.mockResolvedValue({
			available: true,
			reason: null,
			features: ['system-window-menu'],
		});
		const statuses = await collectServiceStatuses();
		expect(statuses['trash']).toEqual({
			available: false,
			reason: 'permission denied',
			features: [],
		});
		expect(statuses['native-dnd']?.available).toBe(false);
		expect(statuses['waypoint-ops']?.features).toEqual(['queue', 'trash']);
		expect(statuses['file-system']?.features).toEqual(['listing']);
		// The Shelf window's always-on-top is among the features the window manager reports (or lacks).
		expect(statuses['window-manager']?.features).toEqual(['system-window-menu']);
	});
});
