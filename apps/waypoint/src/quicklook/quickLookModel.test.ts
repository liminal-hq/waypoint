// Verifies what Quick Look previews each kind of entry as, and where an arrow key steps
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { makeEntry } from '../services/fakeVfsClient';
import { gridMove } from '../browse/gridLayout';
import {
	extensionOf,
	isPreviewable,
	kindMessage,
	neighbourPositions,
	previewKindOf,
	stepTarget,
} from './quickLookModel';

const kindOf = (name: string, overrides = {}) => previewKindOf(makeEntry(1, name, overrides));

describe('previewKindOf', () => {
	it('chooses a kind from what the listing already knows', () => {
		expect(kindOf('photo.png')).toBe('image');
		expect(kindOf('clip.mp4')).toBe('video');
		expect(kindOf('song.mp3')).toBe('audio');
		expect(kindOf('main.rs')).toBe('text');
		expect(kindOf('notes.txt')).toBe('text');
		expect(kindOf('paper.PDF')).toBe('pdf');
		expect(kindOf('face.woff2')).toBe('font');
		expect(kindOf('bundle.zip')).toBe('archive');
		expect(kindOf('folder', { kind: 'directory' })).toBe('folder');
	});

	it('treats a link to a folder as a folder and tries a file with no extension as text', () => {
		expect(kindOf('shortcut', { kind: 'symlink', linkTarget: 'directory' })).toBe('folder');
		expect(kindOf('Makefile')).toBe('text');
		expect(kindOf('thing.bin')).toBe('file');
	});

	it('names every kind with a message', () => {
		expect(kindMessage('pdf', { kind: 'file' })).toBe('quickLook.kind.pdf');
		expect(kindMessage('file', { kind: 'symlink' })).toBe('quickLook.kind.link');
	});
});

describe('extensionOf', () => {
	it('is empty for no extension and for a dotfile', () => {
		expect(extensionOf('a.tar.GZ')).toBe('gz');
		expect(extensionOf('.bashrc')).toBe('');
		expect(extensionOf('README')).toBe('');
	});
});

describe('isPreviewable and neighbourPositions', () => {
	it('skips what is neither a file nor a folder, and the ends of the listing', () => {
		expect(isPreviewable({ kind: 'other' })).toBe(false);
		expect(isPreviewable({ kind: 'symlink' })).toBe(true);
		expect(neighbourPositions(0, 3)).toEqual([1]);
		expect(neighbourPositions(2, 3)).toEqual([1]);
		expect(neighbourPositions(1, 3)).toEqual([2, 0]);
	});
});

describe('stepTarget', () => {
	const list = (key: string, from: number | null) =>
		key === 'ArrowDown' ? (from ?? -1) + 1 : key === 'ArrowUp' ? (from ?? 1) - 1 : null;

	it('steps Left and Right by one in a list, and Up and Down as the view moves', () => {
		expect(stepTarget('ArrowRight', 2, 9, list)).toBe(3);
		expect(stepTarget('ArrowLeft', 2, 9, list)).toBe(1);
		expect(stepTarget('ArrowDown', 2, 9, list)).toBe(3);
		expect(stepTarget('ArrowUp', 2, 9, list)).toBe(1);
	});

	it('stops at either end and ignores other keys', () => {
		expect(stepTarget('ArrowLeft', 0, 9, list)).toBeNull();
		expect(stepTarget('ArrowRight', 9, 9, list)).toBeNull();
		expect(stepTarget('PageDown', 2, 9, list)).toBeNull();
		expect(stepTarget('a', 2, 9, list)).toBeNull();
	});

	it('moves a row at a time with Up and Down in a grid', () => {
		const grid = (key: string, from: number | null, last: number) =>
			gridMove(key, from, last, 4, 3);
		expect(stepTarget('ArrowDown', 1, 11, grid)).toBe(5);
		expect(stepTarget('ArrowUp', 5, 11, grid)).toBe(1);
		expect(stepTarget('ArrowRight', 1, 11, grid)).toBe(2);
		// Already in the last row: no step.
		expect(stepTarget('ArrowDown', 9, 11, grid)).toBeNull();
	});
});
