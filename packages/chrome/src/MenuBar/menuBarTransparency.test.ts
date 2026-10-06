// The menu bar takes the title bar's opacity when asked to, so the two rows match
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const css = readFileSync(join(__dirname, 'MenuBar.module.css'), 'utf8');

describe('the menu bar', () => {
	it('mixes its background with the same opacity token as the title bar when transparent', () => {
		const titleBar = readFileSync(join(__dirname, '..', 'TitleBar', 'TitleBar.module.css'), 'utf8');
		// The title bar's gradient ends in `--wp-title-bar-bottom`, and the menu bar continues from it.
		const mix =
			/color-mix\(\s*in srgb,\s*var\(--wp-title-bar-bottom\) calc\(var\(--wp-title-bar-opacity\) \* 100%\),\s*transparent\s*\)/;
		expect(css).toMatch(/\.bar\[data-transparent\]\s*\{\s*background:\s*color-mix/);
		expect(css).toMatch(mix);
		expect(titleBar).toMatch(mix);
	});
});
