// Glyphs for the Devices section's actions, in the same 16 px stroke style as the rest of the icons
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Glyph, type IconProps } from '../icons/AppIcons';

export const EjectIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M8 3l4.5 6h-9zM3.5 12.25h9" />
	</Glyph>
);

export const UnmountIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M8 2.5v7M5 6.5l3 3 3-3M3 12.25h10" />
	</Glyph>
);

export const MountIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M8 9.5v-7M5 5.5l3-3 3 3M3 12.25h10" />
	</Glyph>
);

export const LockIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect x="3.5" y="7" width="9" height="6.5" rx="1.2" />
		<path d="M5.5 7V5a2.5 2.5 0 015 0v2" />
	</Glyph>
);
