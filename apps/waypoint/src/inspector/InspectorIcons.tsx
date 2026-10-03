// Inline glyphs for the Inspector, in the 16 px stroke style of the other icon sets
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Glyph, type IconProps } from '../icons/AppIcons';

/** A window with a panel down its right side. */
export const InspectorIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M2 3h12v10H2zM10 3v10" />
	</Glyph>
);

/** A circled "i", for Properties. */
export const PropertiesIcon = (props: IconProps) => (
	<Glyph {...props}>
		<circle cx="8" cy="8" r="6" />
		<path d="M8 7.2V11M8 5v.2" />
	</Glyph>
);
