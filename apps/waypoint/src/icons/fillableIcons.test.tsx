// Verifies every outline icon module marks its closed shapes fillable and leaves open strokes alone, so Filled never draws a wedge
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { renderToStaticMarkup } from 'react-dom/server';
import type { ReactElement } from 'react';
import { describe, expect, it } from 'vitest';
import { expectFillableIcons } from '@liminal-hq/waypoint-chrome/icons/expectFillableIcons';
import type { IconGroup } from '@liminal-hq/waypoint-protocol/generated/IconGroup';
import { FileIcon } from '../browse/FileIcon';
import * as deviceIcons from '../devices/DeviceIcons';
import * as inspectorIcons from '../inspector/InspectorIcons';
import * as openWithIcons from '../openWith/OpenWithIcons';
import * as opsIcons from '../ops/OpsIcons';
import * as overviewIcons from '../overview/OverviewIcons';
import * as shelfIcons from '../shelf/ShelfIcons';
import * as pairIcons from '../tabs/PairIcons';
import * as appIcons from './AppIcons';
import * as menuIcons from './MenuIcons';
import { WAYPOINT_FILE_GLYPHS, WAYPOINT_FOLDER_GLYPHS } from './waypointFileIcons';

type Component = (props: object) => ReactElement;

/** Every icon component a module exports, rendered. `Glyph` is the frame and `AppMarkIcon` is the logo's own colours, so neither is an outline icon. */
function rendered(module: Record<string, unknown>): Record<string, string> {
	const out: Record<string, string> = {};
	for (const [name, value] of Object.entries(module)) {
		if (typeof value !== 'function' || name === 'Glyph' || name === 'AppMarkIcon') continue;
		if (name === 'AppIcon') continue;
		const Icon = value as Component;
		out[name] = renderToStaticMarkup(<Icon />);
	}
	return out;
}

describe('every outline icon module is safe under the Filled style', () => {
	describe.each([
		['AppIcons', appIcons, 12],
		['MenuIcons', menuIcons, 50],
		['DeviceIcons', deviceIcons, 4],
		['OpenWithIcons', openWithIcons, 1],
		['OpsIcons', opsIcons, 5],
		['OverviewIcons', overviewIcons, 1],
		['ShelfIcons', shelfIcons, 4],
		['InspectorIcons', inspectorIcons, 3],
		['PairIcons', pairIcons, 2],
	] as const)('%s', (_name, module, atLeast) => {
		it('marks closed shapes fillable and leaves open strokes unmarked', () => {
			const icons = rendered(module);
			expect(Object.keys(icons).length).toBeGreaterThanOrEqual(atLeast);
			expectFillableIcons(icons);
		});
	});

	it('does so in the Waypoint file and folder glyphs', () => {
		const groups = Object.keys(WAYPOINT_FILE_GLYPHS) as IconGroup[];
		const icons: Record<string, string> = {};
		for (const group of groups) {
			icons[group] = renderToStaticMarkup(<FileIcon group={group} theme="waypoint" />);
		}
		for (const special of Object.keys(WAYPOINT_FOLDER_GLYPHS)) {
			icons[`folder:${special}`] = renderToStaticMarkup(
				<FileIcon group="folder" special={special as never} theme="waypoint" />,
			);
		}
		expect(Object.keys(icons).length).toBeGreaterThanOrEqual(41);
		expectFillableIcons(icons);
	});

	it('fills a folder and its standard-folder mark never takes the fill', () => {
		const markup = renderToStaticMarkup(
			<FileIcon group="folder" special="downloads" theme="waypoint" />,
		);
		expect(markup.match(/data-fill/g)).toHaveLength(1);
		expect(markup).toContain('data-outline');
	});
});
