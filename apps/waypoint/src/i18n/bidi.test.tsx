// Verifies addresses stay left to right inside a mirrored layout
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { Ltr, Own, isolateLtr, isolateOwn } from './bidi';

afterEach(() => {
	document.documentElement.dir = '';
});

describe('left-to-right content', () => {
	it('isolates an address as left to right and a name as its own', () => {
		const { container } = render(
			<p>
				<Ltr>sftp://me@nas.lan/srv</Ltr>
				<Own>مجلد</Own>
			</p>,
		);
		const [address, name] = [...container.querySelectorAll('bdi')];
		expect(address).toHaveAttribute('dir', 'ltr');
		expect(name).not.toHaveAttribute('dir');
	});

	it('adds isolates to a value inside a sentence only in a right-to-left window', () => {
		expect(isolateLtr('nas.lan')).toBe('nas.lan');
		expect(isolateOwn('NAS')).toBe('NAS');
		document.documentElement.dir = 'rtl';
		expect(isolateLtr('nas.lan')).toBe('⁦nas.lan⁩');
		expect(isolateOwn('NAS')).toBe('⁨NAS⁩');
		expect(isolateLtr('')).toBe('');
	});
});
