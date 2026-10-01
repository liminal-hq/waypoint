// Verifies the tear-off ghost card draws the payload it receives and stays empty without one
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

type Handler = (_payload: unknown) => void;

const mocks = vi.hoisted(() => ({
	handler: null as Handler | null,
	unlisten: vi.fn(),
	getPayload: vi.fn(),
}));

vi.mock('@liminal-hq/plugin-window-tearoff', () => ({
	onPayload: (handler: Handler) => {
		mocks.handler = handler;
		return Promise.resolve(mocks.unlisten);
	},
	getPayload: () => mocks.getPayload(),
}));

import { readGhostPayload, TearGhostScreen } from './TearGhostScreen';

describe('readGhostPayload', () => {
	it('keeps the fields it knows and drops the rest', () => {
		expect(readGhostPayload({ title: 'Docs', count: 3, label: 'tabs', extra: true })).toEqual({
			title: 'Docs',
			count: 3,
			label: 'tabs',
		});
		expect(readGhostPayload({ title: 4, count: 'x', label: null })).toBeNull();
		expect(readGhostPayload(null)).toBeNull();
		expect(readGhostPayload('text')).toBeNull();
	});
});

describe('TearGhostScreen', () => {
	beforeEach(() => {
		mocks.handler = null;
		mocks.unlisten.mockClear();
		mocks.getPayload.mockReset().mockResolvedValue(null);
	});

	it('draws nothing until a payload arrives', async () => {
		render(<TearGhostScreen />);
		await waitFor(() => expect(mocks.handler).not.toBeNull());
		expect(screen.queryByTestId('tear-ghost-card')).toBeNull();
	});

	it('draws the title, count and label of a payload event', async () => {
		render(<TearGhostScreen />);
		await waitFor(() => expect(mocks.handler).not.toBeNull());
		act(() => mocks.handler?.({ title: 'Documents', count: 2, label: 'Two tabs' }));
		expect(screen.getByText('Documents')).toBeTruthy();
		expect(screen.getByText('2')).toBeTruthy();
		expect(screen.getByText('Two tabs')).toBeTruthy();
		act(() => mocks.handler?.({ title: 'Photos' }));
		expect(screen.getByText('Photos')).toBeTruthy();
		expect(screen.queryByText('Documents')).toBeNull();
	});

	it('picks up a payload that was sent before the page loaded', async () => {
		mocks.getPayload.mockResolvedValue({ title: 'Early', count: 1 });
		render(<TearGhostScreen />);
		expect(await screen.findByText('Early')).toBeTruthy();
	});

	it('stops listening when it unmounts', async () => {
		const { unmount } = render(<TearGhostScreen />);
		await waitFor(() => expect(mocks.handler).not.toBeNull());
		await act(async () => {});
		unmount();
		expect(mocks.unlisten).toHaveBeenCalled();
	});
});
