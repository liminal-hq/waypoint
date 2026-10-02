// The glyph for the Overview place, in the same 16 px stroke style as the rest of the icons
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Glyph, type IconProps } from '../icons/AppIcons';

/** Four tiles: a page of cards. */
export const OverviewIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect x="2.5" y="2.5" width="4.5" height="4.5" rx="1" />
		<rect x="9" y="2.5" width="4.5" height="4.5" rx="1" />
		<rect x="2.5" y="9" width="4.5" height="4.5" rx="1" />
		<rect x="9" y="9" width="4.5" height="4.5" rx="1" />
	</Glyph>
);
