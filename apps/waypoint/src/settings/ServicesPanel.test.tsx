// Verifies the Services panel lists every plugin with its state, features and reason, and survives a failure
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { cleanup, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { collectServiceStatuses, SERVICE_SOURCES } from '../services/serviceStatuses';
import { ServicesPanel } from './ServicesPanel';

afterEach(cleanup);

describe('ServicesPanel', () => {
	it('says it is checking, then lists each plugin with its state, features and reason', async () => {
		render(
			<ServicesPanel
				load={() =>
					Promise.resolve({
						trash: { available: true, reason: null, features: ['trash', 'list'] },
						'native-dnd': {
							available: true,
							reason: 'Outbound drags are not available on this display server',
							features: ['inbound'],
						},
						'system-appearance': { available: false, reason: 'no portal', features: [] },
					})
				}
			/>,
		);
		expect(screen.getByRole('status')).toHaveTextContent('Checking services…');
		const list = await screen.findByRole('list', { name: 'Services' });
		const items = within(list).getAllByRole('listitem');
		expect(items).toHaveLength(3);
		const text = (name: string) => items.find((item) => item.textContent?.includes(name))!;
		expect(text('Trash')).toHaveTextContent('Available');
		expect(text('Trash')).toHaveTextContent('Works: trash, list');
		expect(text('Drag and drop with other apps')).toHaveTextContent('Partly available');
		expect(text('Drag and drop with other apps')).toHaveTextContent(
			'Outbound drags are not available',
		);
		expect(text('System appearance')).toHaveTextContent('Unavailable');
		expect(text('System appearance')).toHaveTextContent('no portal');
	});

	it('lists the plugins alphabetically by the name shown', async () => {
		render(
			<ServicesPanel
				load={() =>
					Promise.resolve({
						trash: { available: true, reason: null, features: [] },
						'file-system': { available: true, reason: null, features: [] },
					})
				}
			/>,
		);
		const names = (await screen.findAllByRole('listitem')).map((item) => item.textContent ?? '');
		expect(names[0]).toContain('File system');
		expect(names[1]).toContain('Trash');
	});

	it('shows an empty list, not a spinner, when the statuses cannot be read at all', async () => {
		render(<ServicesPanel load={() => Promise.reject(new Error('no bridge'))} />);
		expect(await screen.findByRole('list', { name: 'Services' })).toBeEmptyDOMElement();
		expect(screen.queryByRole('status')).toBeNull();
	});
});

describe('the registered sources', () => {
	it('names every plugin, with a message for each', () => {
		expect(Object.keys(SERVICE_SOURCES).sort()).toEqual([
			'file-system',
			'mime-apps',
			'native-dnd',
			'os-prefs',
			'system-appearance',
			'thumbnails',
			'trash',
			'volumes',
			'waypoint-ops',
			'waypoint-session',
			'waypoint-settings',
			'window-effects',
			'window-manager',
			'window-tearoff',
		]);
	});

	it('reports a plugin that cannot answer as unavailable with the error as the reason', async () => {
		const statuses = await collectServiceStatuses();
		for (const status of Object.values(statuses)) {
			expect(status.available).toBe(false);
			expect(status.reason).toBeTruthy();
		}
	});
});
