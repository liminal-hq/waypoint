// Verifies useLocationInfo never reports another location's parent while a lookup is pending or failed
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { FakeVfsClient, fileLocation } from '../services/fakeVfsClient';
import { useLocationInfo } from './locationInfo';

describe('useLocationInfo', () => {
	it('returns nothing for a new location until its own lookup resolves', async () => {
		const client = new FakeVfsClient();
		const a = fileLocation('/a/b');
		const c = fileLocation('/c/d');
		const { result, rerender } = renderHook(({ loc }) => useLocationInfo(client, loc), {
			initialProps: { loc: a },
		});
		await waitFor(() => expect(result.current?.parent?.uri).toBe(fileLocation('/a').uri));

		let release: () => void = () => {};
		const gate = new Promise<void>((resolve) => (release = resolve));
		const original = client.describeLocation.bind(client);
		client.describeLocation = async (location) => {
			await gate;
			return original(location);
		};
		rerender({ loc: c });
		expect(result.current).toBeNull();
		await act(async () => release());
		await waitFor(() => expect(result.current?.parent?.uri).toBe(fileLocation('/c').uri));
	});

	it('keeps the previous answer only when asked to, so a path bar does not flash', async () => {
		const client = new FakeVfsClient();
		const { result, rerender } = renderHook(
			({ loc }) => useLocationInfo(client, loc, { keepPrevious: true }),
			{ initialProps: { loc: fileLocation('/a/b') } },
		);
		await waitFor(() => expect(result.current).not.toBeNull());
		client.describeLocation = () => new Promise(() => {});
		rerender({ loc: fileLocation('/c/d') });
		expect(result.current?.parent?.uri).toBe(fileLocation('/a').uri);
	});
});
