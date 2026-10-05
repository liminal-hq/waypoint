// Verifies which files count as archives, the extensions each format makes, and the names a new archive starts with
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { makeEntry } from '../services/fakeVfsClient';
import {
	archiveTopUri,
	COMPRESS_FORMATS,
	containerUri,
	defaultArchiveName,
	extensionOf,
	isArchiveEntry,
	isArchiveLocation,
	isArchiveName,
	stripArchiveEnd,
	withExtension,
} from './archiveNames';

describe('which names are archives', () => {
	it.each([
		'a.zip',
		'A.ZIP',
		'a.tar',
		'a.tar.gz',
		'a.tgz',
		'a.tar.bz2',
		'a.tbz2',
		'a.tar.xz',
		'a.txz',
		'a.tar.zst',
		'a.7z',
		'book.epub',
		'doc.docx',
		'app.jar',
	])('%s is', (name) => expect(isArchiveName(name)).toBe(true));

	it.each(['a.txt', 'a.gz', 'a.xz', 'zip', 'a.zip.txt', 'a.rar', 'a.tar.lz', ''])(
		'%s is not',
		(name) => expect(isArchiveName(name)).toBe(false),
	);

	it('needs a file: a folder named like an archive is a folder', () => {
		expect(isArchiveEntry(makeEntry(1, 'a.zip'))).toBe(true);
		expect(isArchiveEntry(makeEntry(2, 'a.zip', { kind: 'directory' }))).toBe(false);
		expect(isArchiveEntry(makeEntry(3, 'link.zip', { kind: 'symlink', linkTarget: 'file' }))).toBe(
			true,
		);
		expect(isArchiveEntry(makeEntry(4, 'a.txt'))).toBe(false);
	});
});

describe('archive locations', () => {
	it('tells a place in an archive from any other', () => {
		expect(isArchiveLocation({ display: '', uri: 'archive:file:///a.zip!/' })).toBe(true);
		expect(isArchiveLocation({ display: '', uri: 'ARCHIVE:file:///a.zip!/x' })).toBe(true);
		expect(isArchiveLocation({ display: '', uri: 'file:///a.zip' })).toBe(false);
		expect(isArchiveLocation({ display: '', uri: 'sftp://me@nas/a.zip' })).toBe(false);
	});

	it('writes the top of the archive a file is', () => {
		expect(archiveTopUri({ display: '/a.zip', uri: 'file:///a.zip' })).toBe(
			'archive:file:///a.zip!/',
		);
		// An archive inside an archive.
		expect(archiveTopUri({ display: '', uri: 'archive:file:///a.zip!/b.tar' })).toBe(
			'archive:archive:file:///a.zip!/b.tar!/',
		);
	});

	it('finds the archive file a place is in, however deep and however nested', () => {
		const at = (uri: string) => containerUri({ display: '', uri });
		expect(at('archive:file:///a.zip!/')).toBe('file:///a.zip');
		expect(at('archive:file:///a.zip!/docs/deep')).toBe('file:///a.zip');
		expect(at('archive:archive:file:///a.zip!/b.tar!/x/y')).toBe('archive:file:///a.zip!/b.tar');
		expect(at('archive:sftp://me@nas/srv/a.zip!/')).toBe('sftp://me@nas/srv/a.zip');
		expect(at('file:///a.zip')).toBeNull();
	});
});

describe('names of new archives', () => {
	it('puts the extension of the format on once', () => {
		expect(withExtension('photos', 'zip')).toBe('photos.zip');
		expect(withExtension('photos.zip', 'zip')).toBe('photos.zip');
		expect(withExtension('PHOTOS.ZIP', 'zip')).toBe('PHOTOS.ZIP');
		expect(withExtension('src', 'tarGz')).toBe('src.tar.gz');
		expect(withExtension('src.tar.gz', 'tarXz')).toBe('src.tar.gz.tar.xz');
	});

	it('takes an archive extension off, so a format change changes the end only', () => {
		expect(stripArchiveEnd('src.tar.gz')).toBe('src');
		expect(stripArchiveEnd('a.b.zip')).toBe('a.b');
		expect(stripArchiveEnd('plain')).toBe('plain');
		expect(stripArchiveEnd('.zip')).toBe('.zip');
	});

	it('starts from the one name, or a fallback for many', () => {
		expect(defaultArchiveName(['Report.pdf'], 'Archive')).toBe('Report');
		expect(defaultArchiveName(['folder'], 'Archive')).toBe('folder');
		expect(defaultArchiveName(['.profile'], 'Archive')).toBe('.profile');
		expect(defaultArchiveName(['a', 'b'], 'Archive')).toBe('Archive');
		expect(defaultArchiveName([], 'Archive')).toBe('Archive');
	});

	it('offers every format with a distinct extension', () => {
		const extensions = COMPRESS_FORMATS.map(extensionOf);
		expect(new Set(extensions).size).toBe(COMPRESS_FORMATS.length);
		expect(extensions).toContain('.7z');
		expect(COMPRESS_FORMATS[0]).toBe('zip');
	});
});
