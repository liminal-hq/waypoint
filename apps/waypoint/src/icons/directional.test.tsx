// Tests which glyphs mirror in a right-to-left layout
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { render } from '@testing-library/react';
import type { ReactElement } from 'react';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import {
	BackIcon,
	ChevronLeftIcon,
	ChevronRightSmallIcon,
	ForwardIcon,
	HomeIcon,
	UpIcon,
} from './AppIcons';
import { CopyToIcon, MoveToIcon, RedoIcon, UndoIcon } from './MenuIcons';

const marked = (element: ReactElement): boolean =>
	render(element).container.querySelector('svg')!.hasAttribute('data-directional');

describe('directional glyphs', () => {
	it('marks arrows, chevrons, back and forward and undo and redo', () => {
		for (const icon of [
			<BackIcon />,
			<ForwardIcon />,
			<ChevronRightSmallIcon />,
			<UndoIcon />,
			<RedoIcon />,
			<CopyToIcon />,
			<MoveToIcon />,
		]) {
			expect(marked(icon)).toBe(true);
		}
	});

	it('leaves glyphs that do not point along the line alone, and lets a caller opt out', () => {
		expect(marked(<HomeIcon />)).toBe(false);
		expect(marked(<UpIcon />)).toBe(false);
		expect(marked(<ChevronLeftIcon />)).toBe(false);
		expect(marked(<ChevronRightSmallIcon directional={false} />)).toBe(false);
	});

	it('is mirrored by the base styles under dir=rtl', () => {
		const css = readFileSync(join(import.meta.dirname, '../theme/tokens.css'), 'utf8');
		expect(css).toMatch(/\[dir='rtl'\] svg\[data-directional\] \{\s*scale: -1 1;/);
	});
});
