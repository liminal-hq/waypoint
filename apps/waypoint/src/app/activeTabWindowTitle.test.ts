// Verifies the Main window's OS title never shows a whole path while the folder's name is still being looked up
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { windowTitleFor } from './ActiveTabWindowTitle';

describe('windowTitleFor', () => {
	it('uses the last part of the path while the folder name is not yet known', () => {
		expect(windowTitleFor('/home/scott/Pictures', { display: '/home/scott/Pictures' })).toBe(
			'Pictures',
		);
	});

	it('uses the resolved name once Rust has supplied it', () => {
		expect(windowTitleFor('Home', { display: '/home/scott' })).toBe('Home');
	});

	it('keeps a path that has no last part, such as a root', () => {
		expect(windowTitleFor('/', { display: '/' })).toBe('/');
	});
});
