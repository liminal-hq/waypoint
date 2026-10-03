// Glyphs for a pair: the split mark in the strip, the pane layouts and the pane header's grip
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Glyph, type IconProps } from '../icons/AppIcons';

/** Two panes side by side: the joint between a pair's tabs. */
export const SplitGlyph = (props: IconProps) => (
	<Glyph {...props}>
		<rect data-fill x="2" y="3" width="12" height="10" rx="1.5" />
		<path d="M8 3v10" />
	</Glyph>
);

/** Six dots: where a pane header is grabbed (dragging arrives with the tab drag engine). The dots are solid in every style, so they carry no fill mark. */
export const GripIcon = (props: IconProps) => (
	<Glyph {...props} stroke="none" fill="currentColor">
		<circle cx="6" cy="4" r="1.1" />
		<circle cx="10" cy="4" r="1.1" />
		<circle cx="6" cy="8" r="1.1" />
		<circle cx="10" cy="8" r="1.1" />
		<circle cx="6" cy="12" r="1.1" />
		<circle cx="10" cy="12" r="1.1" />
	</Glyph>
);
