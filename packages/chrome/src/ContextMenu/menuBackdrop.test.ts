// A menu must not carry its own backdrop filter, or its submenus are clipped away
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const css = readFileSync(join(__dirname, 'ContextMenu.module.css'), 'utf8');

/** The declarations of the rule whose selector is exactly `selector`. */
function rule(selector: string): string {
	const found = [...css.matchAll(/([^{}]+)\{([^}]*)\}/g)].find(
		(match) => match[1]!.replace(/\/\*[\s\S]*?\*\//g, '').trim() === selector,
	);
	if (!found) throw new Error(`no rule for ${selector}`);
	return found[2]!;
}

describe('the menu panel', () => {
	it('keeps its blur on a layer behind it, because a backdrop filter on the panel makes it the containing block of the fixed submenus nested inside it', () => {
		expect(rule('.menu')).not.toMatch(/backdrop-filter/);
		expect(rule('.menu::before')).toMatch(/backdrop-filter:\s*var\(--wp-menu-backdrop\)/);
	});
});
