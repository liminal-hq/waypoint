// Inline glyphs for the toolbar, tab strip and status bar, drawn on a 16 px grid in `currentColor`
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

export const BackIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M13 8H3.5M7.5 3.5L3 8l4.5 4.5" />
	</Glyph>
);

export const ForwardIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M3 8h9.5M8.5 3.5L13 8l-4.5 4.5" />
	</Glyph>
);

export const UpIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M8 13V3.5M3.5 7.5L8 3l4.5 4.5" />
	</Glyph>
);

export const PlusIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M8 3v10M3 8h10" />
	</Glyph>
);

export const CloseSmallIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M4 4l8 8M12 4l-8 8" />
	</Glyph>
);

export const ChevronLeftIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M10 3.5L5.5 8l4.5 4.5" />
	</Glyph>
);

export const ChevronRightSmallIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M6 3.5L10.5 8 6 12.5" />
	</Glyph>
);

export const FolderTabIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M1.5 4.5v9h13V6h-7l-1.5-1.5z" />
	</Glyph>
);

export const GridViewIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect x="2" y="2" width="5" height="5" rx="0.5" />
		<rect x="9" y="2" width="5" height="5" rx="0.5" />
		<rect x="2" y="9" width="5" height="5" rx="0.5" />
		<rect x="9" y="9" width="5" height="5" rx="0.5" />
	</Glyph>
);

export const ListViewIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M5.5 4h8M5.5 8h8M5.5 12h8M2.5 4h.01M2.5 8h.01M2.5 12h.01" />
	</Glyph>
);
