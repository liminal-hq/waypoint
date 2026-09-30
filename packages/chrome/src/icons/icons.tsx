// Inline SVG icons used by the window chrome
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ReactNode, SVGProps } from 'react';

type IconProps = Omit<SVGProps<SVGSVGElement>, 'children'> & { size?: number };

function Icon({ size = 16, children, ...rest }: IconProps & { children: ReactNode }) {
	return (
		<svg
			width={size}
			height={size}
			viewBox="0 0 16 16"
			fill="none"
			stroke="currentColor"
			strokeWidth={1.25}
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

export function MinimiseIcon(props: IconProps) {
	return (
		<Icon {...props}>
			<path d="M3.5 8.5h9" />
		</Icon>
	);
}

export function MaximiseIcon(props: IconProps) {
	return (
		<Icon {...props}>
			<rect x="3.5" y="3.5" width="9" height="9" rx="1" />
		</Icon>
	);
}

export function RestoreIcon(props: IconProps) {
	return (
		<Icon {...props}>
			<rect x="3.5" y="5.5" width="7" height="7" rx="1" />
			<path d="M5.5 3.5h6a1 1 0 0 1 1 1v6" />
		</Icon>
	);
}

export function CloseIcon(props: IconProps) {
	return (
		<Icon {...props}>
			<path d="M4 4l8 8M12 4l-8 8" />
		</Icon>
	);
}

export function PinIcon(props: IconProps) {
	return (
		<Icon {...props}>
			<path d="M6 2.5h4M6.5 2.5v4L4.5 9h7l-2-2.5v-4M8 9v4.5" />
		</Icon>
	);
}

export function ChevronRightIcon(props: IconProps) {
	return (
		<Icon {...props}>
			<path d="M6 3.5L10.5 8 6 12.5" />
		</Icon>
	);
}

export function CheckIcon(props: IconProps) {
	return (
		<Icon {...props}>
			<path d="M3.5 8.5l3 3 6-7" />
		</Icon>
	);
}
