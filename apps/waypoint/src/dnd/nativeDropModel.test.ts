// Verifies how the plugin's URIs and paths become a file drag's source: files only, lossless URIs, the folder they share
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import {
	commonFolder,
	droppedFiles,
	externalSource,
	leafOfPath,
	sameUris,
} from './nativeDropModel';

describe('leafOfPath', () => {
	it('is the last part, with either separator and a trailing one', () => {
		expect(leafOfPath('/home/a/report.pdf')).toBe('report.pdf');
		expect(leafOfPath('C:\\Users\\a\\report.pdf')).toBe('report.pdf');
		expect(leafOfPath('/home/a/folder/')).toBe('folder');
		expect(leafOfPath('plain')).toBe('plain');
	});
});

describe('droppedFiles', () => {
	it('keeps the file URIs with their display paths and drops other schemes', () => {
		expect(
			droppedFiles(['file:///a/b%20c.txt', 'https://example.com/x', 'file:///a/d.txt'], []),
		).toEqual([
			{ display: '/a/b c.txt', uri: 'file:///a/b%20c.txt' },
			{ display: '/a/d.txt', uri: 'file:///a/d.txt' },
		]);
		expect(droppedFiles(['https://example.com/x'], ['https://example.com/x'])).toEqual([]);
	});

	it('uses the plugin\u2019s display paths when the lists line up, and keeps the URI untouched', () => {
		expect(droppedFiles(['file:///a/bad%FF.txt'], ['/a/bad\ufffd.txt'])).toEqual([
			{ display: '/a/bad\ufffd.txt', uri: 'file:///a/bad%FF.txt' },
		]);
	});
});

describe('commonFolder', () => {
	const file = (uri: string) => ({ display: uri, uri });

	it('is the folder every file is in, in the lossless spelling', () => {
		expect(commonFolder([file('file:///a/b/x.txt'), file('file:///a/b/y%20z.txt')])).toEqual({
			display: '/a/b',
			uri: 'file:///a/b',
		});
		expect(commonFolder([file('file:///a/sp%20ace/x')])?.uri).toBe('file:///a/sp%20ace');
	});

	it('is null when the files are in different folders, or there are none', () => {
		expect(commonFolder([file('file:///a/x'), file('file:///b/y')])).toBeNull();
		expect(commonFolder([])).toBeNull();
	});

	it('keeps a root\u2019s trailing slash', () => {
		expect(commonFolder([file('file:///x')])?.uri).toBe('file:///');
		expect(commonFolder([file('file:///C:/x.txt')])?.uri).toBe('file:///C:/');
	});

	it('compares folders by what they name, whatever the escapes', () => {
		expect(commonFolder([file('file:///a/b%20c/x'), file('file:///a/b c/y')])).not.toBeNull();
	});
});

describe('sameUris', () => {
	it('is true for the same files in any order and spelling', () => {
		expect(sameUris(['file:///a/b%20c', 'file:///d'], ['file:///d', 'file:///a/b c'])).toBe(true);
		expect(sameUris(['file:///a'], ['file:///b'])).toBe(false);
		expect(sameUris(['file:///a'], ['file:///a', 'file:///b'])).toBe(false);
	});
});

describe('externalSource', () => {
	it('names one file and counts several, with plain pages for the stack', () => {
		const one = externalSource([{ display: '/srv/a b.txt', uri: 'file:///srv/a%20b.txt' }]);
		expect(one).toMatchObject({
			count: 1,
			name: 'a b.txt',
			groups: ['document'],
			session: null,
			handle: null,
			spec: null,
			readOnly: false,
			rightButton: false,
			folder: { uri: 'file:///srv' },
			external: { own: false },
		});
		const many = externalSource(
			Array.from({ length: 5 }, (_, i) => ({ display: `/s/${i}`, uri: `file:///s/${i}` })),
			true,
		);
		expect(many.count).toBe(5);
		expect(many.name).toBeNull();
		expect(many.groups).toHaveLength(3);
		expect(many.external?.own).toBe(true);
	});
});
