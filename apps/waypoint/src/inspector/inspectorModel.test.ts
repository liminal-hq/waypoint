// Verifies the Inspector's rules: preview kinds, the facts' wording and the debounce constants
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import {
	baseName,
	DETAILS_DELAY_MS,
	formatPermissions,
	HEAVY_DELAY_MS,
	isFolderKind,
	kindMessage,
	permissionString,
	previewKind,
} from './inspectorModel';

describe('previewKind', () => {
	it('shows an image, a sound or a video by its group until the content type is known', () => {
		expect(previewKind('image', null)).toBe('image');
		expect(previewKind('audio', null)).toBe('audio');
		expect(previewKind('video', null)).toBe('video');
		expect(previewKind('code', null)).toBe('text');
	});

	it('does not read a document or an unknown file as text before something says it is', () => {
		expect(previewKind('document', null)).toBe('none');
		expect(previewKind('other', null)).toBe('none');
		expect(previewKind('document', 'text/plain')).toBe('text');
	});

	it('lets the content type decide once it is known', () => {
		expect(previewKind('other', 'image/svg+xml')).toBe('image');
		expect(previewKind('other', 'application/json')).toBe('text');
		expect(previewKind('other', 'application/vnd.api+json')).toBe('text');
		expect(previewKind('document', 'application/pdf')).toBe('none');
		expect(previewKind('code', 'application/octet-stream')).toBe('none');
		expect(previewKind('other', 'audio/ogg')).toBe('audio');
		expect(previewKind('other', 'video/webm')).toBe('video');
	});
});

describe('the facts', () => {
	it('writes permissions as rwx and octal', () => {
		expect(permissionString(0o755)).toBe('rwxr-xr-x');
		expect(permissionString(0o640)).toBe('rw-r-----');
		expect(formatPermissions(0o644)).toBe('rw-r--r-- (0644)');
	});

	it('shows setuid, setgid and sticky in the execute positions', () => {
		expect(permissionString(0o4755)).toBe('rwsr-xr-x');
		expect(permissionString(0o4644)).toBe('rwSr--r--');
		expect(permissionString(0o2755)).toBe('rwxr-sr-x');
		expect(permissionString(0o1777)).toBe('rwxrwxrwt');
		expect(permissionString(0o1776)).toBe('rwxrwxrwT');
		expect(formatPermissions(0o5755)).toBe('rwsr-xr-t (5755)');
	});

	it('names the last part of a path, or a root whole', () => {
		expect(baseName('/home/test/docs')).toBe('docs');
		expect(baseName('/home/test/docs/')).toBe('docs');
		expect(baseName('/')).toBe('/');
	});

	it('knows a folder, and a link that leads to one', () => {
		expect(isFolderKind('directory', null)).toBe(true);
		expect(isFolderKind('symlink', 'directory')).toBe(true);
		expect(isFolderKind('symlink', 'file')).toBe(false);
		expect(isFolderKind('file', null)).toBe(false);
	});

	it('says what an entry is', () => {
		expect(kindMessage('directory', 'folder')).toBe('inspector.kind.folder');
		expect(kindMessage('symlink', 'image')).toBe('inspector.kind.link');
		expect(kindMessage('file', 'image')).toBe('inspector.kind.image');
		expect(kindMessage('file', 'other')).toBe('inspector.kind.file');
	});
});

describe('the delays', () => {
	it('read the cheap facts inside the 50 ms budget and wait longer for the heavy ones', () => {
		expect(DETAILS_DELAY_MS).toBeLessThan(50);
		expect(HEAVY_DELAY_MS).toBeGreaterThan(DETAILS_DELAY_MS);
	});
});
