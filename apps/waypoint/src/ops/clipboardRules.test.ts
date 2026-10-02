// Verifies the clipboard's rules: which rows a cut dims, what a paste refuses and asks for, and the rule table for which clipboard wins
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import type { Clipboard, Location } from '../services/opsClient';
import type { OsFiles } from '../services/osClipboardClient';
import {
	chooseClipboard,
	clipboardAsFiles,
	cutNames,
	isWithin,
	leafName,
	normaliseUri,
	parentUri,
	pasteRefusal,
	pasteRequest,
	sameFiles,
} from './clipboardRules';

const at = (path: string): Location => ({
	display: path,
	uri: `file://${path.split('/').map(encodeURIComponent).join('/')}`,
});
const board = (
	mode: Clipboard['mode'],
	paths: string[],
	source: Clipboard['source'] = 'app',
): Clipboard => ({ mode, items: paths.map(at), source, revision: 1 });

describe('locations as text', () => {
	it('drops trailing slashes and decodes escapes, so two spellings of a place are one', () => {
		expect(normaliseUri('file:///a/b/')).toBe('file:///a/b');
		expect(normaliseUri('file:///a/b%20c')).toBe('file:///a/b c');
		expect(normaliseUri('file:///')).toBe('file:///');
		expect(normaliseUri('file:///a/%E0%A4%A')).toBe('file:///a/%E0%A4%A');
	});

	it('finds the parent and the name of a uri, and no parent for a root', () => {
		expect(parentUri('file:///a/b/c.txt')).toBe('file:///a/b');
		expect(parentUri('file:///a')).toBe('file:///');
		expect(parentUri('file:///')).toBeNull();
		expect(parentUri('file:///a/b%20c/d')).toBe('file:///a/b c');
		expect(leafName('file:///a/b%20c.txt')).toBe('b c.txt');
		expect(leafName('file:///a/folder/')).toBe('folder');
	});

	it('knows a place from what lies inside it, and never mistakes a longer name for it', () => {
		expect(isWithin('file:///a/b', 'file:///a')).toBe(true);
		expect(isWithin('file:///a', 'file:///a')).toBe(true);
		expect(isWithin('file:///ab', 'file:///a')).toBe(false);
		expect(isWithin('file:///a', 'file:///a/b')).toBe(false);
		expect(isWithin('file:///a', 'file:///')).toBe(true);
	});
});

describe('which rows a cut dims', () => {
	it('dims the cut items that sit directly in the folder, by name', () => {
		const cut = board('cut', ['/home/a/one.txt', '/home/a/two', '/home/b/three', '/home/a/x/four']);
		expect([...cutNames(cut, 'file:///home/a')].sort()).toEqual(['one.txt', 'two']);
		expect([...cutNames(cut, 'file:///home/b/')]).toEqual(['three']);
		expect(cutNames(cut, 'file:///elsewhere').size).toBe(0);
	});

	it('dims nothing for a copy or an empty clipboard', () => {
		expect(cutNames(board('copy', ['/home/a/one.txt']), 'file:///home/a').size).toBe(0);
		expect(cutNames(board('cut', []), 'file:///home/a').size).toBe(0);
	});

	it('matches a name with a space or an escape in it', () => {
		const cut = board('cut', ['/home/a/my file.txt']);
		expect([...cutNames(cut, 'file:///home/a')]).toEqual(['my file.txt']);
	});
});

describe('what a paste refuses', () => {
	const into = at('/home/dest');

	it('refuses a folder pasted into itself or into a folder inside it', () => {
		expect(pasteRefusal('copy', [at('/home/dest')], into)).toEqual({ kind: 'intoItself' });
		expect(pasteRefusal('cut', [at('/home')], into)).toEqual({ kind: 'intoItself' });
		expect(
			pasteRefusal('copy', [at('/home/x'), at('/home/dest/sub')], at('/home/dest/sub/deep')),
		).toEqual({ kind: 'intoItself' });
	});

	it('refuses a cut whose every item is already in the destination, but not a mixed one', () => {
		expect(pasteRefusal('cut', [at('/home/dest/a'), at('/home/dest/b')], into)).toEqual({
			kind: 'sameFolder',
		});
		expect(pasteRefusal('cut', [at('/home/dest/a'), at('/home/src/b')], into)).toBeNull();
	});

	it('lets a copy into its own folder through, where it becomes a duplicate', () => {
		expect(pasteRefusal('copy', [at('/home/dest/a')], into)).toBeNull();
	});

	it('lets an ordinary paste through, and a longer name is not the folder itself', () => {
		expect(pasteRefusal('copy', [at('/home/src/a')], into)).toBeNull();
		expect(pasteRefusal('cut', [at('/home/des')], into)).toBeNull();
	});
});

describe('the job a paste makes', () => {
	const into = at('/home/dest');

	it('moves a cut and copies a copy, into the destination, with no conflict policy so nothing is overwritten unasked', () => {
		const cut = pasteRequest(board('cut', ['/home/src/a']), into, 'main-3');
		expect(cut).toEqual({
			kind: { kind: 'move' },
			sources: { kind: 'locations', locations: [at('/home/src/a')] },
			destination: into,
			name: null,
			options: { conflict: null, verify: null },
			originWindow: 'main-3',
		});
		const copy = pasteRequest(board('copy', ['/home/src/a', '/home/src/b']), into, 'main-3');
		expect(copy.kind).toEqual({ kind: 'copy' });
		expect(copy.destination).toEqual(into);
	});

	it('makes a duplicate of a copy pasted beside its originals, as other file managers do', () => {
		const request = pasteRequest(board('copy', ['/home/dest/a', '/home/dest/b']), into, 'main-1');
		expect(request.kind).toEqual({ kind: 'duplicate' });
		expect(request.destination).toBeNull();
		// A mix is an ordinary copy: the planner skips the ones that are already there.
		expect(
			pasteRequest(board('copy', ['/home/dest/a', '/home/src/b']), into, 'main-1').kind,
		).toEqual({ kind: 'copy' });
	});
});

describe('files on the system clipboard', () => {
	it('shares only the file: items of the clipboard, with its intent', () => {
		const mixed: Clipboard = {
			mode: 'cut',
			items: [at('/a/b'), { display: 'remote', uri: 'sftp://host/x' }],
			source: 'app',
			revision: 3,
		};
		expect(clipboardAsFiles(mixed)).toEqual({ uris: [at('/a/b').uri], cut: true });
		expect(clipboardAsFiles(board('copy', []))).toBeNull();
		expect(clipboardAsFiles({ ...mixed, items: [mixed.items[1]!] })).toBeNull();
	});

	it('compares file lists by their files and intent, not their order or spelling', () => {
		const a: OsFiles = { uris: ['file:///x/a%20b', 'file:///x/c/'], cut: false };
		expect(sameFiles(a, { uris: ['file:///x/c', 'file:///x/a b'], cut: false })).toBe(true);
		expect(sameFiles(a, { uris: ['file:///x/c', 'file:///x/a b'], cut: true })).toBe(false);
		expect(sameFiles(a, { uris: ['file:///x/c'], cut: false })).toBe(false);
		expect(sameFiles(null, null)).toBe(true);
		expect(sameFiles(a, null)).toBe(false);
	});
});

describe('which clipboard a paste uses', () => {
	const app = board('copy', ['/home/app/a'], 'app');
	const appFiles = clipboardAsFiles(app)!;
	const other: OsFiles = { uris: ['file:///home/other/z'], cut: false };
	const otherCut: OsFiles = { uris: ['file:///home/other/z'], cut: true };

	// One row per case of the rule in `chooseClipboard`.
	const table: Array<{
		name: string;
		available: boolean;
		os: OsFiles | null;
		lastSeen: OsFiles | null;
		expected: 'app' | 'os';
	}> = [
		{
			name: 'the system clipboard is unavailable',
			available: false,
			os: other,
			lastSeen: null,
			expected: 'app',
		},
		{
			name: 'the system clipboard holds nothing',
			available: true,
			os: null,
			lastSeen: null,
			expected: 'app',
		},
		{
			name: 'the system clipboard holds something that is not files',
			available: true,
			os: null,
			lastSeen: other,
			expected: 'app',
		},
		{
			name: 'an empty file list',
			available: true,
			os: { uris: [], cut: false },
			lastSeen: null,
			expected: 'app',
		},
		{
			name: 'it holds the same files Waypoint copied',
			available: true,
			os: appFiles,
			lastSeen: null,
			expected: 'app',
		},
		{
			name: 'it holds files already seen, and Waypoint has copied since',
			available: true,
			os: other,
			lastSeen: other,
			expected: 'app',
		},
		{
			name: 'another application copied files since',
			available: true,
			os: other,
			lastSeen: null,
			expected: 'os',
		},
		{
			name: 'another application copied different files since the last look',
			available: true,
			os: other,
			lastSeen: appFiles,
			expected: 'os',
		},
		{
			name: 'another application cut files',
			available: true,
			os: otherCut,
			lastSeen: other,
			expected: 'os',
		},
	];

	it.each(table)('$name -> $expected', ({ available, os, lastSeen, expected }) => {
		expect(chooseClipboard({ available, app, os, lastSeen })).toBe(expected);
	});

	it('stands on Waypoint’s clipboard when it is empty and the system’s holds nothing', () => {
		expect(
			chooseClipboard({
				available: true,
				app: board('copy', []),
				os: null,
				lastSeen: null,
			}),
		).toBe('app');
	});

	it('adopts the system clipboard when Waypoint’s is empty and another application copied', () => {
		expect(
			chooseClipboard({ available: true, app: board('copy', []), os: other, lastSeen: null }),
		).toBe('os');
	});
});
