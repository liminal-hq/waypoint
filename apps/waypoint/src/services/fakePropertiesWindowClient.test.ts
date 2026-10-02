// Verifies the fake keeps the factory's rules: four at most, the same subject reuses its window, a closed window frees its place
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { FakePropertiesWindowClient } from './fakePropertiesWindowClient';
import { fileLocation } from './fakeVfsClient';
import { MAX_PROPERTIES_WINDOWS, subjectKey } from './propertiesWindowClient';

const at = (path: string) => fileLocation(path);

describe('subjectKey', () => {
	it('ignores a trailing slash and keeps a root', () => {
		expect(subjectKey('file:///home/me/')).toBe('file:///home/me');
		expect(subjectKey('file:///home/me')).toBe('file:///home/me');
		expect(subjectKey('file:///')).toBe('file:///');
	});
});

describe('the fake Properties window client', () => {
	it('opens a window for a new subject', async () => {
		const client = new FakePropertiesWindowClient();
		expect(await client.open(at('/a'))).toBe('opened');
		expect([...client.windows.keys()]).toEqual(['properties-1']);
	});

	it('focuses the window that has the subject instead of making another', async () => {
		const client = new FakePropertiesWindowClient();
		await client.open(at('/a'));
		expect(await client.open(at('/a'))).toBe('focused');
		expect(client.windows.size).toBe(1);
		expect(client.focused).toEqual(['properties-1']);
	});

	it('refuses a fifth subject, but still focuses one that has a window', async () => {
		const client = new FakePropertiesWindowClient();
		for (const name of ['a', 'b', 'c', 'd']) await client.open(at(`/${name}`));
		expect(client.windows.size).toBe(MAX_PROPERTIES_WINDOWS);
		expect(await client.open(at('/e'))).toBe('limit');
		expect(client.windows.size).toBe(MAX_PROPERTIES_WINDOWS);
		expect(await client.open(at('/c'))).toBe('focused');
	});

	it('frees a place when a window closes', async () => {
		const client = new FakePropertiesWindowClient();
		for (const name of ['a', 'b', 'c', 'd']) await client.open(at(`/${name}`));
		client.close('properties-2');
		expect(await client.open(at('/e'))).toBe('opened');
	});

	it('answers what the window is about, and follows a rename', async () => {
		const client = new FakePropertiesWindowClient(at('/a'));
		expect((await client.subject()).uri).toBe(at('/a').uri);
		await client.setSubject(at('/b'));
		expect((await client.subject()).uri).toBe(at('/b').uri);
		await expect(new FakePropertiesWindowClient().subject()).rejects.toThrow();
	});
});
