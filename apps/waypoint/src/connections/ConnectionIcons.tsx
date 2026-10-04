// Glyphs for servers and connections, in the 16 px stroke style of `AppIcons`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Glyph, type IconProps } from '../icons/AppIcons';

/** A server: two stacked units with a light each. */
export const ServerIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect data-fill x="2.5" y="2.5" width="11" height="4.5" rx="1" />
		<rect data-fill x="2.5" y="9" width="11" height="4.5" rx="1" />
		<path d="M5 4.75h.01M5 11.25h.01M8 4.75h3.5M8 11.25h3.5" />
	</Glyph>
);

/** Disconnect: a plug pulled apart. */
export const DisconnectIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M3 13l2.5-2.5M13 3l-2.5 2.5" />
		<path data-fill d="M5.5 10.5l-1-1 2.5-2.5 2 2-2.5 2.5zM10.5 5.5l1 1-2.5 2.5-2-2 2.5-2.5z" />
	</Glyph>
);

/** Reconnect: a circular arrow. */
export const ReconnectIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M13 8a5 5 0 1 1-1.5-3.55M13 2.5v2.5h-2.5" />
	</Glyph>
);
