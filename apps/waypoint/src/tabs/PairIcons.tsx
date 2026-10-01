// Glyphs for a pair: the split mark in the strip, the pane layouts and the pane header's grip
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ReactNode, SVGProps } from 'react';

type IconProps = Omit<SVGProps<SVGSVGElement>, 'children'>;

function Glyph({ children, ...rest }: IconProps & { children: ReactNode }) {
	return (
		<svg
			width={16}
			height={16}
			viewBox="0 0 16 16"
			fill="none"
			stroke="currentColor"
			strokeWidth={1.4}
			strokeLinecap="round"
			strokeLinejoin="round"
			aria-hidden="true"
			focusable="false"
			{...rest}
		>
			{children}
		</svg>
	);
}

/** Two panes side by side: the joint between a pair's tabs. */
export const SplitGlyph = (props: IconProps) => (
	<Glyph {...props}>
		<rect x="2" y="3" width="12" height="10" rx="1.5" />
		<path d="M8 3v10" />
	</Glyph>
);

/** Six dots: where a pane header is grabbed (dragging arrives with the tab drag engine). */
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
