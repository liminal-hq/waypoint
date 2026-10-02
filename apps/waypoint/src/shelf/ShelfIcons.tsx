// Inline glyphs for the Shelf, in the 16 px stroke style of the other icon sets
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Glyph, type IconProps } from '../icons/AppIcons';

/** A shelf with two things standing on it. */
export const ShelfIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M2 13.5h12M3.5 13.5V9.5h3v4M8 13.5V6h3v7.5M12.5 13.5v-6" />
	</Glyph>
);

/** A page with a plus, for Add to Shelf. */
export const AddToShelfIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M2 13.5h12M4 13.5V8h4v5.5M11 3.5v4M9 5.5h4" />
	</Glyph>
);
