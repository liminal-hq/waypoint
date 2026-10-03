// The Portage icon for an entry: a coloured file or folder drawn from static art
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useMemo } from 'react';
import { portageIconSvg, type PortageIconOptions } from './portageIcons';
import styles from './PortageIcon.module.css';

interface PortageIconProps extends PortageIconOptions {
	/** The edge in pixels. The art is square and scales cleanly from 16 up. */
	size?: number;
	/** An extra class, for a view that draws the icon at a size the list does not. */
	className?: string;
}

/** The inside of an `<svg>`: everything between its opening and closing tags. */
function innerMarkup(svg: string): string {
	return svg.slice(svg.indexOf('>') + 1, svg.lastIndexOf('</svg>'));
}

/**
 * The Portage artwork for an entry's icon group (and standard folder). Decorative: the row's name
 * carries the meaning. The art is trusted, static SVG that ships with the app, never file content,
 * so it is set as markup.
 */
export function PortageIcon({
	group,
	special,
	colour,
	tone,
	variant,
	badge,
	size = 16,
	className,
}: PortageIconProps) {
	const markup = useMemo(
		() => innerMarkup(portageIconSvg({ group, special, colour, tone, variant, badge })),
		[group, special, colour, tone, variant, badge],
	);
	const marked = group === 'folder' && special ? special : undefined;
	return (
		<svg
			className={className ? `${styles.icon} ${className}` : styles.icon}
			data-group={group}
			data-special={marked}
			width={size}
			height={size}
			viewBox="0 0 64 64"
			fill="none"
			aria-hidden="true"
			focusable="false"
			dangerouslySetInnerHTML={{ __html: markup }}
		/>
	);
}
