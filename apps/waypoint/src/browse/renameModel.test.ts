// Verifies what inline rename starts with selected and when the extension guard asks
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { extensionChange, initialSelection, splitName, withExtension } from './renameModel';

describe('splitName', () => {
	it('splits where a person would, like `split_name` in Rust', () => {
		expect(splitName('a.txt')).toEqual({ stem: 'a', extension: '.txt' });
		expect(splitName('a.b.txt')).toEqual({ stem: 'a.b', extension: '.txt' });
		expect(splitName('a.tar.gz')).toEqual({ stem: 'a', extension: '.tar.gz' });
		expect(splitName('A.TAR.xz')).toEqual({ stem: 'A', extension: '.TAR.xz' });
		expect(splitName('plain')).toEqual({ stem: 'plain', extension: '' });
	});

	it('treats a dotfile and a trailing dot as all stem', () => {
		expect(splitName('.bashrc')).toEqual({ stem: '.bashrc', extension: '' });
		expect(splitName('.profile')).toEqual({ stem: '.profile', extension: '' });
		expect(splitName('end.')).toEqual({ stem: 'end.', extension: '' });
		expect(splitName('.tar.gz')).toEqual({ stem: '.tar', extension: '.gz' });
	});
});

describe('initialSelection', () => {
	it('stops before the extension of a file', () => {
		expect(initialSelection('photo.jpg', false)).toEqual([0, 5]);
		expect(initialSelection('archive.tar.gz', false)).toEqual([0, 7]);
	});

	it('takes the whole name of a folder, a dotfile and a file with no extension', () => {
		expect(initialSelection('v1.2', true)).toEqual([0, 4]);
		expect(initialSelection('.bashrc', false)).toEqual([0, 7]);
		expect(initialSelection('Makefile', false)).toEqual([0, 8]);
	});
});

describe('extensionChange', () => {
	it('reports a changed or removed extension of a file', () => {
		expect(extensionChange('a.jpg', 'a.png', false)).toEqual({ from: '.jpg', to: '.png' });
		expect(extensionChange('a.jpg', 'a', false)).toEqual({ from: '.jpg', to: '' });
		expect(extensionChange('a.tar.gz', 'a.tar.xz', false)).toEqual({
			from: '.tar.gz',
			to: '.tar.xz',
		});
	});

	it('does not ask about a rename that keeps the extension, only changes its case, or a folder', () => {
		expect(extensionChange('a.jpg', 'b.jpg', false)).toBeNull();
		expect(extensionChange('a.JPG', 'a.jpg', false)).toBeNull();
		expect(extensionChange('v1.2', 'v1.3', true)).toBeNull();
	});

	it('does not ask about giving an extensionless file one, or about a dotfile', () => {
		expect(extensionChange('Makefile', 'Makefile.bak', false)).toBeNull();
		expect(extensionChange('.bashrc', '.zshrc', false)).toBeNull();
	});
});

describe('withExtension', () => {
	it('puts the original extension back on what was typed', () => {
		expect(withExtension('holiday.png', '.jpg')).toBe('holiday.jpg');
		expect(withExtension('holiday', '.jpg')).toBe('holiday.jpg');
	});
});
