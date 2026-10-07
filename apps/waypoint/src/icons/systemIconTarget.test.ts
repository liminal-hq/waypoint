// Verifies which system icon an entry asks for: the extension, a group's stand-in type or a kind of folder, and the size and scale it asks at
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import {
	extensionOf,
	GROUP_TYPES,
	hasOwnIcon,
	ICON_SIZES,
	iconScale,
	iconSizeFor,
	systemIconTarget,
} from './systemIconTarget';

describe('extensionOf', () => {
	it('is the last extension in lower case', () => {
		expect(extensionOf('Report.PDF')).toBe('pdf');
		expect(extensionOf('backup.tar.gz')).toBe('gz');
		expect(extensionOf('a.b+c')).toBe('b+c');
	});

	it('is nothing for a name a type cannot go by', () => {
		for (const name of [undefined, '', 'Makefile', '.bashrc', 'trailing.', '...', 'a.b/c', 'a.é']) {
			expect(extensionOf(name), String(name)).toBeNull();
		}
		expect(extensionOf(`name.${'x'.repeat(25)}`)).toBeNull();
	});
});

describe('systemIconTarget', () => {
	it('asks for a file by its extension, so every file of a type is one request', () => {
		expect(systemIconTarget('pdf', 'one.pdf')).toEqual({ extension: 'pdf' });
		expect(systemIconTarget('pdf', 'two.PDF')).toEqual({ extension: 'pdf' });
		// The extension wins over the group, which is only the stand-in.
		expect(systemIconTarget('other', 'photo.jpg')).toEqual({ extension: 'jpg' });
	});

	it('falls back to the group’s stand-in type with no extension to go by', () => {
		expect(systemIconTarget('code', 'Makefile')).toEqual({ mime: 'text/x-csrc' });
		expect(systemIconTarget('image')).toEqual({ mime: 'image/x-generic' });
		expect(systemIconTarget('other', '.hidden')).toEqual({ mime: 'application/octet-stream' });
	});

	it('has a stand-in for every group but folders, as the generated union lists them', () => {
		const text = readFileSync(
			join(import.meta.dirname, '../../../../packages/protocol/src/generated/IconGroup.ts'),
			'utf8',
		);
		const groups = [...text.slice(text.indexOf('export type')).matchAll(/"([A-Za-z0-9]+)"/g)].map(
			(match) => match[1]!,
		);
		expect(Object.keys(GROUP_TYPES).sort()).toEqual(groups.filter((g) => g !== 'folder').sort());
		for (const type of Object.values(GROUP_TYPES))
			expect(type).toMatch(/^[a-z0-9.+-]+\/[a-z0-9.+-]+$/);
	});

	it('asks for a folder by its kind: the standard ones by name and the rest as a plain folder', () => {
		expect(systemIconTarget('folder', 'Downloads', 'downloads')).toEqual({ folder: 'downloads' });
		expect(systemIconTarget('folder', 'Home', 'home')).toEqual({ folder: 'home' });
		expect(systemIconTarget('folder', 'src')).toEqual({ folder: 'plain' });
		expect(systemIconTarget('folder', 'src', null)).toEqual({ folder: 'plain' });
		// Projects has a mark in the Waypoint sets only.
		expect(systemIconTarget('folder', 'Projects', 'projects')).toEqual({ folder: 'plain' });
		// A folder named like a file is still a folder.
		expect(systemIconTarget('folder', 'archive.zip')).toEqual({ folder: 'plain' });
	});
});

describe('the size and scale asked for', () => {
	it('rounds a size up to the next bucket, so a few requests serve every view', () => {
		expect(iconSizeFor(16)).toBe(16);
		expect(iconSizeFor(17)).toBe(24);
		expect(iconSizeFor(40)).toBe(48);
		expect(iconSizeFor(64)).toBe(64);
		expect(iconSizeFor(300)).toBe(256);
		expect(iconSizeFor(Number.NaN)).toBe(16);
		for (let size = 1; size <= 256; size += 1) expect(ICON_SIZES).toContain(iconSizeFor(size));
	});

	it('asks at a whole device pixel ratio from 1 to 3', () => {
		expect(iconScale(1)).toBe(1);
		expect(iconScale(1.25)).toBe(2);
		expect(iconScale(2)).toBe(2);
		expect(iconScale(4)).toBe(3);
		expect(iconScale(0.5)).toBe(1);
		expect(iconScale(Number.NaN)).toBe(1);
	});
});

describe('hasOwnIcon', () => {
	it('is true for the files that carry their own icon, in any case', () => {
		for (const name of [
			'setup.exe',
			'SETUP.EXE',
			'a.b.exe',
			'x.ico',
			'x.cur',
			'x.ani',
			'x.scr',
			'Notes.LNK',
		]) {
			expect(hasOwnIcon(name), name).toBe(true);
		}
	});

	it('is false for every other name', () => {
		for (const name of [
			undefined,
			'',
			'exe',
			'.exe',
			'report.pdf',
			'app.exe.txt',
			'setup.msi',
			'a.exe/',
		]) {
			expect(hasOwnIcon(name), String(name)).toBe(false);
		}
	});

	it('lists the extensions the plugin draws from the file, which decides what is drawn', () => {
		const text = readFileSync(
			join(import.meta.dirname, '../../../../plugins/mime-apps/src/typeicons.rs'),
			'utf8',
		);
		const list = /OWN_ICON_EXTENSIONS: \[&str; \d+\] = \[([^\]]*)\]/.exec(text)?.[1] ?? '';
		const extensions = [...list.matchAll(/"([a-z0-9]+)"/g)].map((match) => match[1]!);
		expect(extensions.length).toBeGreaterThan(0);
		for (const extension of extensions) expect(hasOwnIcon(`a.${extension}`), extension).toBe(true);
		expect(hasOwnIcon('a.zzz')).toBe(false);
		// Nothing the page asks by file is left out by the plugin.
		for (const extension of ['exe', 'ico', 'cur', 'ani', 'scr', 'lnk'])
			expect(extensions, extension).toContain(extension);
	});
});
