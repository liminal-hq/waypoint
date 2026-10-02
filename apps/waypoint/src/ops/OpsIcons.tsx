// Glyphs for the queue ring and its rows
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Glyph, type IconProps } from '../icons/AppIcons';

/** Three stacked lines: the quiet state of the ring, with nothing queued. */
export const OpsQueueIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M3 4.5h10M3 8h10M3 11.5h6" />
	</Glyph>
);

export const OpsCheckIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M3.5 8.5l3 3 6-7" />
	</Glyph>
);

export const OpsPopOutIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M9.5 3H13v3.5M13 3L7.5 8.5M6 4H3.5v8.5H12V10" />
	</Glyph>
);

export const OpsUpIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M4 9.5L8 5.5l4 4" />
	</Glyph>
);

export const OpsDownIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M4 6.5l4 4 4-4" />
	</Glyph>
);
