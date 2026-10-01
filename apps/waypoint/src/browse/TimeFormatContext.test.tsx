// Tests for the hour cycle the views read: its first read, a live change and an unavailable plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { FakeTimeFormatClient } from '../services/fakeTimeFormatClient';
import { TimeFormatProvider, useHourCycle } from './TimeFormatContext';

function Probe() {
	return <output>{useHourCycle() ?? 'locale'}</output>;
}

afterEach(cleanup);

describe('useHourCycle', () => {
	it('is undefined without a client, leaving the choice to Intl', () => {
		render(
			<TimeFormatProvider>
				<Probe />
			</TimeFormatProvider>,
		);
		expect(screen.getByRole('status').textContent).toBe('locale');
	});

	it('reads the system setting once it loads', async () => {
		const client = new FakeTimeFormatClient('h23');
		render(
			<TimeFormatProvider client={client}>
				<Probe />
			</TimeFormatProvider>,
		);
		expect(await screen.findByText('h23')).toBeTruthy();
		expect(client.reads).toBe(1);
	});

	it('follows a change made while the app runs', async () => {
		const client = new FakeTimeFormatClient('h23');
		render(
			<TimeFormatProvider client={client}>
				<Probe />
			</TimeFormatProvider>,
		);
		await screen.findByText('h23');
		act(() => client.set('h12'));
		expect(await screen.findByText('h12')).toBeTruthy();
		act(() => client.set('h23'));
		expect(await screen.findByText('h23')).toBeTruthy();
	});

	it('falls back to the locale when the plugin cannot answer', async () => {
		const client = new FakeTimeFormatClient('h23');
		client.failReads();
		render(
			<TimeFormatProvider client={client}>
				<Probe />
			</TimeFormatProvider>,
		);
		await act(async () => {});
		expect(screen.getByRole('status').textContent).toBe('locale');
	});

	it('lets a change that arrives first win over the read in flight', async () => {
		const client = new FakeTimeFormatClient('h23');
		let resolveRead: (value: 'h23' | 'h12' | undefined) => void = () => {};
		client.get = () => new Promise((resolve) => (resolveRead = resolve));
		render(
			<TimeFormatProvider client={client}>
				<Probe />
			</TimeFormatProvider>,
		);
		act(() => client.set('h12'));
		await act(async () => resolveRead('h23'));
		expect(screen.getByRole('status').textContent).toBe('h12');
	});

	it('stops listening when the window goes away', async () => {
		const client = new FakeTimeFormatClient('h23');
		const { unmount } = render(
			<TimeFormatProvider client={client}>
				<Probe />
			</TimeFormatProvider>,
		);
		await screen.findByText('h23');
		unmount();
		expect(() => client.set('h12')).not.toThrow();
	});
});
