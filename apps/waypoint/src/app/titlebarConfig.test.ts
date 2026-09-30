// Verifies the platform fallback for the title bar's style, layout and alignment
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { fallbackTitlebarConfig } from './titlebarConfig';

describe('fallbackTitlebarConfig', () => {
	it('uses caption buttons and a left-aligned title on Windows', () => {
		const config = fallbackTitlebarConfig('windows');
		expect(config.controlsStyle).toBe('win11');
		expect(config.titleAlign).toBe('start');
		expect(config.buttonLayout.end).toEqual(['minimise', 'maximise', 'close']);
	});

	it('uses the GNOME style and a centred title on Linux and unknown platforms', () => {
		for (const platform of ['linux', undefined, 'macos']) {
			const config = fallbackTitlebarConfig(platform);
			expect(config.controlsStyle).toBe('gnome');
			expect(config.titleAlign).toBe('center');
		}
	});
});
