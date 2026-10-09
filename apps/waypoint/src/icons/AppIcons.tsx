// Inline glyphs for the toolbar, tab strip and status bar, drawn on a 16 px grid in `currentColor`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useId, type ReactNode, type SVGProps } from 'react';
import styles from './AppIcons.module.css';

export type IconProps = Omit<SVGProps<SVGSVGElement>, 'children'> & {
	/** Overrides whether the glyph mirrors in a right-to-left layout; each directional glyph sets it. */
	directional?: boolean;
};

/**
 * A glyph on the 16 px grid. A directional one (an arrow, a chevron, back and forward, undo and
 * redo) is marked `data-directional`, and the base styles flip it under `dir=rtl`. One that stands
 * for a physical direction (scrolling a strip to its left) opts out with `directional={false}`.
 */
export function Glyph({
	children,
	directional,
	className,
	...rest
}: IconProps & { children: ReactNode; directional?: boolean }) {
	return (
		<svg
			className={className ? `${styles.glyph} ${className}` : styles.glyph}
			data-directional={directional ? '' : undefined}
			width={16}
			height={16}
			viewBox="0 0 16 16"
			fill="none"
			stroke="currentColor"
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
	<Glyph directional {...props}>
		<path d="M13 8H3.5M7.5 3.5L3 8l4.5 4.5" />
	</Glyph>
);

export const ForwardIcon = (props: IconProps) => (
	<Glyph directional {...props}>
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
	<Glyph directional {...props}>
		<path d="M6 3.5L10.5 8 6 12.5" />
	</Glyph>
);

export const FolderTabIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path data-fill d="M1.5 4.5v9h13V6h-7l-1.5-1.5z" />
	</Glyph>
);

export const GridViewIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect data-fill x="2" y="2" width="5" height="5" rx="0.5" />
		<rect data-fill x="9" y="2" width="5" height="5" rx="0.5" />
		<rect data-fill x="2" y="9" width="5" height="5" rx="0.5" />
		<rect data-fill x="9" y="9" width="5" height="5" rx="0.5" />
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
	<Glyph directional {...props}>
		<rect data-fill x="2" y="3" width="12" height="10" rx="1.5" />
		<path d="M6 3v10" />
	</Glyph>
);

export const StarIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path data-fill d="M8 2l1.8 3.7 4 .6-2.9 2.8.7 4L8 11.2 4.4 13.1l.7-4L2.2 6.3l4-.6z" />
	</Glyph>
);

export const PinIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M9.5 2.5l4 4-2 .8-2.2 2.2.4 2.6-1 1L5.5 9.5 2.5 13.5M6.5 5.5l-1 1 4 4" />
	</Glyph>
);

/** A shield with a tick: Administrator mode, on a tab and in the title bar (always beside the word, never alone). */
export const ShieldIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path data-fill d="M8 1.8l5 1.8v4.2c0 3-2.1 5-5 6.4-2.9-1.4-5-3.4-5-6.4V3.6z" />
		<path d="M5.8 8l1.6 1.6 3-3.2" />
	</Glyph>
);

/** The app logo beside the app menu's name: the icon's dark tile, folder outline and waypoint ring, in the logo's own colours (the same drawing as `src-tauri/icons/source/waypoint.svg`). */
export function AppMarkIcon({ width = 16, height = 16, ...rest }: IconProps) {
	const gradient = useId();
	return (
		<svg
			width={width}
			height={height}
			viewBox="0 0 64 64"
			aria-hidden="true"
			focusable="false"
			{...rest}
		>
			<defs>
				<linearGradient
					id={gradient}
					x1="12"
					y1="10"
					x2="52"
					y2="54"
					gradientUnits="userSpaceOnUse"
				>
					<stop offset="0" stopColor="#F4B860" />
					<stop offset="1" stopColor="#D4551C" />
				</linearGradient>
			</defs>
			<rect x="2" y="2" width="60" height="60" rx="16" fill="#0E0D12" />
			<path
				d="M14 24a4 4 0 014-4h8l4 5h16a4 4 0 014 4v15a4 4 0 01-4 4H18a4 4 0 01-4-4z"
				fill="none"
				stroke={`url(#${gradient})`}
				strokeWidth={3.4}
				strokeLinejoin="round"
			/>
			<circle cx="32" cy="35" r="6.2" fill="none" stroke="#F7E2BA" strokeWidth={2.4} />
			<circle cx="32" cy="35" r="2.2" fill="#F7E2BA" />
		</svg>
	);
}
