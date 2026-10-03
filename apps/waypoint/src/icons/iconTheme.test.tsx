// Verifies how the icon set, folder colour and tone are chosen for a window and how entry icons follow them live
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { act, cleanup, render, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { FileIcon } from '../browse/FileIcon';
import { DEFAULT_SETTINGS } from '../services/settingsClient';
import { applyAppearance, NO_OS_APPEARANCE, resolveAppearance } from '../theme/appearance';
import { iconLook, readIconLook, resolveIconTheme } from './iconTheme';
import { FOLDER_COLOURS } from './portage/portagePalette';

const root = document.documentElement;

function clearRoot(): void {
	for (const key of ['iconTheme', 'folderColour', 'theme']) delete root.dataset[key];
}

beforeEach(clearRoot);
afterEach(() => {
	cleanup();
	clearRoot();
});

/** The string literals of a generated union type, read from the file Rust generates. */
function literals(name: string): string[] {
	const file = join(
		import.meta.dirname,
		'../../../../packages/protocol/src/generated',
		`${name}.ts`,
	);
	const text = readFileSync(file, 'utf8');
	const union = text.slice(text.indexOf('export type'));
	return [...union.matchAll(/"([A-Za-z0-9]+)"/g)].map((match) => match[1]!);
}

describe('the folder colours', () => {
	it('are the same ten, in the same order, as the settings Rust accepts', () => {
		expect(literals('FolderColour')).toEqual([...FOLDER_COLOURS]);
	});

	it('and the icon themes the settings accept are the three the page and views know', () => {
		expect(literals('IconTheme')).toEqual(['waypoint', 'portage', 'system']);
	});
});

describe('resolveIconTheme', () => {
	it('draws Portage as Portage and everything else, System included, as Waypoint', () => {
		expect(resolveIconTheme('portage')).toBe('portage');
		expect(resolveIconTheme('waypoint')).toBe('waypoint');
		expect(resolveIconTheme('system')).toBe('system');
	});
});

describe('iconLook', () => {
	it('takes the tone from the resolved scheme, and from the OS only when there is none', () => {
		expect(iconLook('portage', 'red', 'dark', false).tone).toBe('dark');
		expect(iconLook('portage', 'red', 'light', true).tone).toBe('light');
		expect(iconLook('portage', 'red', undefined, true).tone).toBe('dark');
		expect(iconLook('portage', 'red', undefined, false).tone).toBe('light');
	});

	it('falls back to the Waypoint set in Liminal for a missing or unknown value', () => {
		expect(iconLook(undefined, undefined, 'light', false)).toEqual({
			theme: 'waypoint',
			colour: 'liminal',
			tone: 'light',
		});
		expect(iconLook('neon', 'teal', 'light', false)).toMatchObject({
			theme: 'waypoint',
			colour: 'liminal',
		});
	});
});

describe('the attributes the theme engine writes', () => {
	it('carry the resolved theme and the colour onto the root, and System is its own set', () => {
		const settings = (iconTheme: 'waypoint' | 'portage' | 'system') => ({
			...DEFAULT_SETTINGS,
			appearance: {
				...DEFAULT_SETTINGS.appearance,
				mode: 'dark' as const,
				iconTheme,
				folderColour: 'kde' as const,
			},
		});
		const options = { touchPointer: false, systemLanguage: 'en-CA', hasFinePointer: true };
		applyAppearance(root, resolveAppearance(settings('portage'), NO_OS_APPEARANCE, options));
		expect(readIconLook()).toEqual({ theme: 'portage', colour: 'kde', tone: 'dark' });
		applyAppearance(root, resolveAppearance(settings('system'), NO_OS_APPEARANCE, options));
		expect(root.dataset.iconTheme).toBe('system');
		expect(readIconLook().theme).toBe('system');
	});
});

describe('FileIcon', () => {
	const set = (theme: string, colour: string, scheme: string): void => {
		root.dataset.iconTheme = theme;
		root.dataset.folderColour = colour;
		root.dataset.theme = scheme;
	};

	it('draws the Waypoint glyphs by default', () => {
		const { container } = render(<FileIcon group="pdf" />);
		const svg = container.querySelector('svg')!;
		expect(svg.getAttribute('viewBox')).toBe('0 0 16 16');
		expect(svg.dataset.group).toBe('pdf');
	});

	it.each([
		['dark', 'dark'],
		['light', 'light'],
	])('draws Portage in the chosen colour in the %s scheme with the %s tone', (scheme, tone) => {
		set('portage', 'purple', scheme);
		const { container } = render(<FileIcon group="folder" special="music" />);
		const svg = container.querySelector('svg')!;
		expect(svg.getAttribute('viewBox')).toBe('0 0 64 64');
		expect(svg.dataset.special).toBe('music');
		expect(svg.getAttribute('aria-hidden')).toBe('true');
		expect(svg.dataset.colour).toBe('purple');
		expect(svg.dataset.tone).toBe(tone);
	});

	it('draws a file in Portage, not by colour', () => {
		set('portage', 'red', 'light');
		const { container } = render(<FileIcon group="image" />);
		expect(container.querySelector('svg')!.getAttribute('viewBox')).toBe('0 0 64 64');
		expect(container.querySelector('svg')!.dataset.colour).toBeUndefined();
	});

	it('gives a different folder drawing for each colour and each tone', () => {
		const draw = (colour: string, scheme: string): string => {
			set('portage', colour, scheme);
			const { container, unmount } = render(<FileIcon group="folder" />);
			const html = container.innerHTML;
			unmount();
			return html;
		};
		const all = new Set<string>();
		for (const colour of FOLDER_COLOURS) {
			for (const scheme of ['dark', 'light']) all.add(draw(colour, scheme));
		}
		expect(all.size).toBe(FOLDER_COLOURS.length * 2);
	});

	it('follows a change of theme, colour and scheme live, in the icons already mounted', async () => {
		const { container } = render(<FileIcon group="folder" />);
		const svg = container.querySelector('svg')!;
		expect(svg.getAttribute('viewBox')).toBe('0 0 16 16');
		act(() => {
			set('portage', 'red', 'dark');
		});
		await waitFor(() =>
			expect(container.querySelector('svg')!.getAttribute('viewBox')).toBe('0 0 64 64'),
		);
		const dark = container.innerHTML;
		act(() => {
			root.dataset.theme = 'light';
		});
		await waitFor(() => expect(container.innerHTML).not.toBe(dark));
		const light = container.innerHTML;
		act(() => {
			root.dataset.folderColour = 'pink';
		});
		await waitFor(() => expect(container.innerHTML).not.toBe(light));
		act(() => {
			root.dataset.iconTheme = 'waypoint';
		});
		await waitFor(() =>
			expect(container.querySelector('svg')!.getAttribute('viewBox')).toBe('0 0 16 16'),
		);
	});

	it('takes an override, so a preview can draw a set the window is not set to', () => {
		const { container } = render(
			<FileIcon group="folder" theme="portage" colour="gnome" tone="light" />,
		);
		expect(container.querySelector('svg')!.getAttribute('viewBox')).toBe('0 0 64 64');
		const svg = container.querySelector('svg')!;
		expect(svg.dataset.colour).toBe('gnome');
		expect(svg.dataset.tone).toBe('light');
	});

	it('passes a size class to either set', () => {
		const { container } = render(<FileIcon group="pdf" className="big" />);
		expect(container.querySelector('svg')!.getAttribute('class')).toMatch(/\bbig\b/);
		set('portage', 'liminal', 'light');
		const portage = render(<FileIcon group="pdf" className="big" />);
		expect(portage.container.querySelector('svg')!.getAttribute('class')).toMatch(/\bbig\b/);
	});
});
