// The Waypoint icon set for files and folders: one outline glyph per icon group and per standard folder
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { IconGroup } from '@liminal-hq/waypoint-protocol/generated/IconGroup';
import type { SpecialFolder } from '@liminal-hq/waypoint-protocol/generated/SpecialFolder';
import type { ReactNode } from 'react';

// Each glyph is drawn in a 16 by 16 box and is the children of an `<svg>` whose stylesheet sets
// `currentColor` strokes, no fill and round joins. Rust decides the group; these tables only map
// it to a shape. `Record` keys make a missing group a compile error.
const PAGE = 'M4 1.5h5.5L13 5v9.5H4z M9.5 1.5V5H13';
const FOLDER = 'M1.5 4.5v9h13V6h-7l-1.5-1.5z';

export const WAYPOINT_FILE_GLYPHS: Record<IconGroup, ReactNode> = {
	folder: <path d={FOLDER} />,
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
	code: <path d="M6 4.5L2.5 8 6 11.5M10 4.5L13.5 8 10 11.5" />,
	document: (
		<>
			<path d={PAGE} />
			<path d="M6 8.5h5M6 10.5h5M6 12.5h3" />
		</>
	),
	other: <path d={PAGE} />,
	pdf: (
		<>
			<path d={PAGE} />
			<path d="M6.5 13V8.5h1.4a1.3 1.3 0 010 2.6H6.5" />
		</>
	),
	app: (
		<>
			<rect x="2" y="3" width="12" height="10" rx="1" />
			<path d="M2 6h12M4 4.5h.01M6 4.5h.01" />
		</>
	),
	text: (
		<>
			<path d={PAGE} />
			<path d="M6.5 8.5h4M8.5 8.5v4.5" />
		</>
	),
	markdown: (
		<>
			<rect x="1.5" y="3.5" width="13" height="9" rx="1" />
			<path d="M4 10.5v-5l2 2.5 2-2.5v5M11 5.5v5M9.5 9l1.5 1.5L12.5 9" />
		</>
	),
	spreadsheet: (
		<>
			<rect x="2" y="2" width="12" height="12" rx="1" />
			<path d="M2 6h12M2 10h12M6.5 2v12" />
		</>
	),
	presentation: (
		<>
			<rect x="2" y="2.5" width="12" height="8" rx="1" />
			<path d="M4.5 8.5l2-2 2 1.5 3-3M8 10.5v3M5.5 14h5" />
		</>
	),
	font: <path d="M3.5 13.5L8 2.5l4.5 11M5.4 9.5h5.2" />,
	diskImage: (
		<>
			<circle cx="8" cy="8" r="6" />
			<circle cx="8" cy="8" r="1.6" />
		</>
	),
	database: (
		<>
			<ellipse cx="8" cy="4" rx="5" ry="2" />
			<path d="M3 4v8c0 1.1 2.2 2 5 2s5-.9 5-2V4M3 8c0 1.1 2.2 2 5 2s5-.9 5-2" />
		</>
	),
	config: (
		<>
			<path d="M2 5h2.2M6.8 5H14M2 8h7.2M11.8 8H14M2 11h3.7M8.3 11H14" />
			<circle cx="5.5" cy="5" r="1.3" />
			<circle cx="10.5" cy="8" r="1.3" />
			<circle cx="7" cy="11" r="1.3" />
		</>
	),
	shellScript: (
		<>
			<rect x="1.5" y="3" width="13" height="10" rx="1" />
			<path d="M4.5 6.5l2 1.5-2 1.5M8 10.5h3" />
		</>
	),
	executable: (
		<>
			<rect x="2" y="2" width="12" height="12" rx="1.5" />
			<path d="M6.5 5.5l4 2.5-4 2.5z" />
		</>
	),
	certificate: (
		<>
			<circle cx="8" cy="6.5" r="3.5" />
			<path d="M6 9.5L5 14.5l3-1.5 3 1.5-1-5" />
		</>
	),
	ebook: (
		<path d="M2 3.5c2-.7 4-.7 6 .8 2-1.5 4-1.5 6-.8v9c-2-.7-4-.7-6 .8-2-1.5-4-1.5-6-.8zM8 4.3v9" />
	),
	torrent: (
		<path d="M3.5 13.5V7a4.5 4.5 0 019 0v6.5h-3V7a1.5 1.5 0 00-3 0v6.5zM3.5 11h3M9.5 11h3" />
	),
	calendar: (
		<>
			<rect x="2" y="3" width="12" height="11" rx="1" />
			<path d="M2 6.5h12M5.5 1.5v3M10.5 1.5v3" />
		</>
	),
	contact: (
		<>
			<rect x="1.5" y="3" width="13" height="10" rx="1" />
			<circle cx="5.5" cy="7.2" r="1.5" />
			<path d="M3.5 11c0-1.2 1-1.8 2-1.8s2 .6 2 1.8M9.5 7h3M9.5 9.5h3" />
		</>
	),
	log: (
		<>
			<path d={PAGE} />
			<path d="M6 8.5h.01M8 8.5h3M6 10.5h.01M8 10.5h3M6 12.5h.01M8 12.5h3" />
		</>
	),
	model3d: (
		<>
			<path d="M8 1.5l5.5 3v7L8 14.5l-5.5-3v-7z" />
			<path d="M2.5 4.5L8 7.5l5.5-3M8 7.5v7" />
		</>
	),
	subtitles: (
		<>
			<rect x="1.5" y="3" width="13" height="10" rx="1" />
			<path d="M4 8.5h3M8.5 8.5H12M5.5 10.8h5" />
		</>
	),
	playlist: (
		<>
			<path d="M2 4h9M2 7h9M2 10h5" />
			<path d="M10 10.5v3.5l4-1.75z" />
		</>
	),
	package: (
		<>
			<rect x="1.5" y="3" width="13" height="3" />
			<path d="M2.5 6v7.5h11V6M6.5 8.5h3" />
		</>
	),
	symlink: <path d="M3 12.5v-3a3 3 0 013-3h7M10 3.5l3.5 3-3.5 3" />,
};

// A standard folder is the plain folder shape with a small mark inside, drawn without a fill so the
// folder's own tint shows around it.
const mark = (children: ReactNode): ReactNode => (
	<>
		<path d={FOLDER} />
		<g fill="none">{children}</g>
	</>
);

export const WAYPOINT_FOLDER_GLYPHS: Record<SpecialFolder, ReactNode> = {
	home: mark(<path d="M5.5 10.5L8 8l2.5 2.5M6.3 9.7v2.3h3.4V9.7" />),
	desktop: mark(
		<>
			<rect x="5.6" y="7.6" width="4.8" height="3" rx=".4" />
			<path d="M7 12h2" />
		</>,
	),
	documents: mark(<path d="M6.5 8.5h3M6.5 10h3M6.5 11.5h2" />),
	downloads: mark(<path d="M8 7.8v3.8M6.3 10l1.7 1.7L9.7 10" />),
	pictures: mark(
		<>
			<path d="M5.8 12l1.9-2.4 1.5 1.5 1-1 1.1 1.9" />
			<circle cx="6.8" cy="8.6" r=".6" />
		</>,
	),
	music: mark(
		<>
			<path d="M7.1 11V8.3l2.9-.6v2.8" />
			<circle cx="6.2" cy="11" r=".9" />
			<circle cx="9.1" cy="10.5" r=".9" />
		</>,
	),
	videos: mark(<path d="M6.8 8.1v3.4l3.2-1.7z" />),
	templates: mark(<rect x="5.8" y="7.8" width="4.4" height="3.8" strokeDasharray="1.2 1" />),
	public: mark(
		<>
			<circle cx="8" cy="8.6" r="1" />
			<path d="M5.8 12.2c0-1.4 1-2 2.2-2s2.2.6 2.2 2" />
		</>,
	),
	projects: mark(<path d="M6 8v3.6M8 8v2.4M10 8v3.6" />),
};
