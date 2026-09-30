// The bundled icon set: one small SVG glyph per icon group, until theme lookup and thumbnails arrive
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { IconGroup } from '@liminal-hq/waypoint-protocol/generated/IconGroup';
import type { ReactNode } from 'react';
import styles from './FileIcon.module.css';

// Each glyph is drawn in a 16 by 16 box with `currentColor`, so the stylesheet colours it. Rust
// decides the group; this table only maps it to a shape.
const PAGE = 'M4 1.5h5.5L13 5v9.5H4z M9.5 1.5V5H13';

const GLYPHS: Record<IconGroup, ReactNode> = {
	folder: <path d="M1.5 4.5v9h13V6h-7l-1.5-1.5z" />,
	image: (
		<>
			<rect x="2" y="3" width="12" height="10" rx="1" />
			<path d="M2.5 11.5l3.5-3.5 3 3 2-2 2.5 2.5" />
			<circle cx="10.5" cy="6" r="1" />
		</>
	),
	audio: (
		<>
			<path d="M6 11.5V3l6-1.5v8.5" />
			<circle cx="4.5" cy="11.5" r="1.5" />
			<circle cx="10.5" cy="10" r="1.5" />
		</>
	),
	video: (
		<>
			<rect x="1.5" y="3.5" width="9" height="9" rx="1" />
			<path d="M10.5 7l4-2v6l-4-2" />
		</>
	),
	archive: (
		<>
			<rect x="2" y="2" width="12" height="12" rx="1" />
			<path d="M8 2v6M7 4h2M7 6h2" />
			<rect x="6.5" y="8" width="3" height="3" />
		</>
	),
	code: (
		<>
			<path d="M6 4.5L2.5 8 6 11.5M10 4.5L13.5 8 10 11.5" />
		</>
	),
	document: (
		<>
			<path d={PAGE} />
			<path d="M6 8.5h5M6 10.5h5M6 12.5h3" />
		</>
	),
	other: <path d={PAGE} />,
};

interface FileIconProps {
	group: IconGroup;
}

/** The glyph for an entry's icon group. Decorative: the row's name carries the meaning. */
export function FileIcon({ group }: FileIconProps) {
	return (
		<svg
			className={styles.icon}
			data-group={group}
			viewBox="0 0 16 16"
			aria-hidden="true"
			focusable="false"
		>
			{GLYPHS[group]}
		</svg>
	);
}
