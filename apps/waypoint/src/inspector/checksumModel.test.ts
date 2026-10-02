// Verifies who may be hashed, when the size warning shows and how progress reads
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { CHECKSUM_ALGORITHMS } from '../services/checksumClient';
import {
	algorithmMessage,
	canChecksum,
	DEFAULT_ALGORITHM,
	fraction,
	LARGE_FILE_BYTES,
	needsSizeWarning,
} from './checksumModel';

describe('canChecksum', () => {
	it('offers a regular file and a link that leads to one', () => {
		expect(canChecksum('file', null)).toBe(true);
		expect(canChecksum('symlink', 'file')).toBe(true);
	});

	it('never offers a folder, a link to a folder, a broken link or a special file', () => {
		expect(canChecksum('directory', null)).toBe(false);
		expect(canChecksum('symlink', 'directory')).toBe(false);
		expect(canChecksum('symlink', null)).toBe(false);
		expect(canChecksum('other', null)).toBe(false);
	});
});

describe('needsSizeWarning', () => {
	it('warns above 1 GiB and not at it', () => {
		expect(needsSizeWarning(LARGE_FILE_BYTES)).toBe(false);
		expect(needsSizeWarning(LARGE_FILE_BYTES + 1)).toBe(true);
		expect(needsSizeWarning(10)).toBe(false);
		expect(needsSizeWarning(null)).toBe(false);
	});
});

describe('the algorithms', () => {
	it('start as SHA-256 and are the ones verification offers', () => {
		expect(DEFAULT_ALGORITHM).toBe('sha256');
		expect(CHECKSUM_ALGORITHMS).toEqual(['sha256', 'blake3']);
		expect(algorithmMessage('blake3')).toBe('checksum.algorithm.blake3');
	});
});

describe('fraction', () => {
	it('is the share read, held between 0 and 1, and 0 for a file of no size', () => {
		expect(fraction(50, 200)).toBe(0.25);
		expect(fraction(300, 200)).toBe(1);
		expect(fraction(1, 0)).toBe(0);
	});
});
