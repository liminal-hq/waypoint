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

export const HomeIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M2 8L8 2.5 14 8M3.5 7v6.5h3.5V10h2v3.5h3.5V7" />
	</Glyph>
);

export const SidebarIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect x="2" y="3" width="12" height="10" rx="1.5" />
		<path d="M6 3v10" />
	</Glyph>
);

export const StarIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M8 2l1.8 3.7 4 .6-2.9 2.8.7 4L8 11.2 4.4 13.1l.7-4L2.2 6.3l4-.6z" />
	</Glyph>
);

export const PinIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M9.5 2.5l4 4-2 .8-2.2 2.2.4 2.6-1 1L5.5 9.5 2.5 13.5M6.5 5.5l-1 1 4 4" />
	</Glyph>
);
