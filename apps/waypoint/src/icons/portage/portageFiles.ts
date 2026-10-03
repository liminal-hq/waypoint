// The Portage file-type artwork: one static SVG per type, imported as text
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import appSvg from './files/app.svg?raw';
import archiveSvg from './files/archive.svg?raw';
import audioSvg from './files/audio.svg?raw';
import blankSvg from './files/blank.svg?raw';
import calendarSvg from './files/calendar.svg?raw';
import certificateSvg from './files/certificate.svg?raw';
import codeSvg from './files/code.svg?raw';
import configSvg from './files/config.svg?raw';
import contactSvg from './files/contact.svg?raw';
import databaseSvg from './files/database.svg?raw';
import diskImageSvg from './files/disk-image.svg?raw';
import documentSvg from './files/document.svg?raw';
import ebookSvg from './files/ebook.svg?raw';
import executableSvg from './files/executable.svg?raw';
import fontSvg from './files/font.svg?raw';
import imageSvg from './files/image.svg?raw';
import logSvg from './files/log.svg?raw';
import markdownSvg from './files/markdown.svg?raw';
import model3dSvg from './files/model-3d.svg?raw';
import packageSvg from './files/package.svg?raw';
import pdfSvg from './files/pdf.svg?raw';
import playlistSvg from './files/playlist.svg?raw';
import presentationSvg from './files/presentation.svg?raw';
import shellScriptSvg from './files/shell-script.svg?raw';
import spreadsheetSvg from './files/spreadsheet.svg?raw';
import subtitlesSvg from './files/subtitles.svg?raw';
import symlinkSvg from './files/symlink.svg?raw';
import textSvg from './files/text.svg?raw';
import torrentSvg from './files/torrent.svg?raw';
import unknownSvg from './files/unknown.svg?raw';
import videoSvg from './files/video.svg?raw';

// Each file is a standalone 64 by 64 SVG with its own gradient and filter ids, so any number can be
// inlined in one page. They are trusted, static art that ships with the app.
export const PORTAGE_FILE_SVGS = {
	app: appSvg,
	archive: archiveSvg,
	audio: audioSvg,
	blank: blankSvg,
	calendar: calendarSvg,
	certificate: certificateSvg,
	code: codeSvg,
	config: configSvg,
	contact: contactSvg,
	database: databaseSvg,
	'disk-image': diskImageSvg,
	document: documentSvg,
	ebook: ebookSvg,
	executable: executableSvg,
	font: fontSvg,
	image: imageSvg,
	log: logSvg,
	markdown: markdownSvg,
	'model-3d': model3dSvg,
	package: packageSvg,
	pdf: pdfSvg,
	playlist: playlistSvg,
	presentation: presentationSvg,
	'shell-script': shellScriptSvg,
	spreadsheet: spreadsheetSvg,
	subtitles: subtitlesSvg,
	symlink: symlinkSvg,
	text: textSvg,
	torrent: torrentSvg,
	unknown: unknownSvg,
	video: videoSvg,
} as const;

export type PortageFileName = keyof typeof PORTAGE_FILE_SVGS;

const SHADOW_FILTER = /<filter id="[^"]*-sh"[^>]*>[\s\S]*?<\/filter>\n?/;
const SHADOW_ELLIPSE = /<ellipse [^>]*filter="url\(#[^"]*-sh\)"[^>]*\/>\n?/;
const EMPTY_DEFS = /<defs>\s*<\/defs>\n?/;

/** The art without its shadow: the blur filter and the ellipse that uses it, and any `<defs>` left empty. */
function withoutShadow(svg: string): string {
	return svg.replace(SHADOW_FILTER, '').replace(SHADOW_ELLIPSE, '').replace(EMPTY_DEFS, '');
}

const SHADOWLESS = new Map<PortageFileName, string>();

/**
 * One file type's SVG. The shadow (a blurred ellipse under the page, drawn with an SVG filter) is left
 * out unless asked for: it costs a filter pass per icon, which a long listing feels. The shadowless
 * string is built once per type and the same instance is returned thereafter.
 */
export function portageFileSvg(name: PortageFileName, shadow = false): string {
	if (shadow) return PORTAGE_FILE_SVGS[name];
	let svg = SHADOWLESS.get(name);
	if (svg === undefined) {
		svg = withoutShadow(PORTAGE_FILE_SVGS[name]);
		SHADOWLESS.set(name, svg);
	}
	return svg;
}
