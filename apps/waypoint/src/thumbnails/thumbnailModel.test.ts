// Verifies which entries get thumbnails, the size asked for, and the order positions are asked in
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { makeEntry } from '../services/fakeVfsClient';
import {
	entryThumbKey,
	listShowsThumbnails,
	listThumbnailPixels,
	thumbnailPositions,
	thumbSizeFor,
	wantsThumbnail,
} from './thumbnailModel';

describe('wantsThumbnail', () => {
	it('asks about pictures, documents, video and files of an unknown kind', () => {
		for (const name of ['a.jpg', 'a.pdf', 'a.mp4', 'a.txt', 'a.unknown']) {
			expect(wantsThumbnail(makeEntry(1, name))).toBe(true);
		}
	});

	it('leaves folders, sounds, archives and source files with their icons', () => {
		expect(wantsThumbnail(makeEntry(1, 'dir', { kind: 'directory' }))).toBe(false);
		for (const name of ['a.mp3', 'a.zip', 'a.rs']) {
			expect(wantsThumbnail(makeEntry(1, name))).toBe(false);
		}
	});

	it('asks about a link only when it points at a file', () => {
		expect(wantsThumbnail(makeEntry(1, 'l.jpg', { kind: 'symlink', linkTarget: 'file' }))).toBe(
			true,
		);
		expect(
			wantsThumbnail(makeEntry(1, 'l.jpg', { kind: 'symlink', linkTarget: 'directory' })),
		).toBe(false);
		expect(wantsThumbnail(makeEntry(1, 'l.jpg', { kind: 'symlink', linkTarget: null }))).toBe(
			false,
		);
		expect(wantsThumbnail(makeEntry(1, 'sock', { kind: 'other' }))).toBe(false);
	});
});

describe('entryThumbKey', () => {
	it('changes with the modified time, so a changed file is asked for again', () => {
		const before = makeEntry(7, 'a.jpg', { modifiedMs: 1 });
		const after = makeEntry(7, 'a.jpg', { modifiedMs: 2 });
		expect(entryThumbKey(before)).not.toBe(entryThumbKey(after));
		expect(entryThumbKey(makeEntry(7, 'a.jpg', { modifiedMs: null }))).toBe('7:0');
	});
});

describe('thumbSizeFor', () => {
	it('picks the smallest standard size that is not scaled up', () => {
		expect(thumbSizeFor(64)).toBe('normal');
		expect(thumbSizeFor(128)).toBe('normal');
		expect(thumbSizeFor(129)).toBe('large');
		expect(thumbSizeFor(256)).toBe('large');
		expect(thumbSizeFor(300)).toBe('x-large');
	});

	it('counts device pixels, so a sharp screen gets a bigger one', () => {
		expect(thumbSizeFor(96, 1)).toBe('normal');
		expect(thumbSizeFor(96, 2)).toBe('large');
		expect(thumbSizeFor(256, 2)).toBe('x-large');
		expect(thumbSizeFor(600, 2)).toBe('xx-large');
	});
});

describe('the list', () => {
	it('shows pictures only in rows tall enough for them', () => {
		expect(listShowsThumbnails(24)).toBe(false);
		expect(listShowsThumbnails(28)).toBe(false);
		expect(listShowsThumbnails(34)).toBe(false);
		expect(listShowsThumbnails(44)).toBe(true);
		expect(listThumbnailPixels(44)).toBe(36);
	});
});

describe('thumbnailPositions', () => {
	it('asks for what is in view first, then a screen after it, then a screen before it nearest first', () => {
		const { visible, margin } = thumbnailPositions(10, 12, 100);
		expect(visible).toEqual([10, 11, 12]);
		expect(margin).toEqual([13, 14, 15, 9, 8, 7]);
	});

	it('stops at both ends of the listing', () => {
		expect(thumbnailPositions(0, 2, 5)).toEqual({ visible: [0, 1, 2], margin: [3, 4] });
		expect(thumbnailPositions(3, 4, 5)).toEqual({ visible: [3, 4], margin: [2, 1] });
		expect(thumbnailPositions(0, 3, 0)).toEqual({ visible: [], margin: [] });
	});

	it('clamps a range that runs past the end', () => {
		expect(thumbnailPositions(3, 9, 5).visible).toEqual([3, 4]);
	});
});
