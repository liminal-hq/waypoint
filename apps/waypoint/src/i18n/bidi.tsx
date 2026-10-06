// Keeps addresses, URLs, paths and host names left to right inside a layout that may be mirrored
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ReactNode } from 'react';

/**
 * Text that is left-to-right content wherever it sits: an address (`sftp://me@nas.lan/srv`), a URL,
 * a path or a host name. It is isolated from the text around it, so a parenthesis or a colon beside
 * it stays on the side it was written on in a right-to-left layout.
 */
export function Ltr({ children }: { children: ReactNode }) {
	return <bdi dir="ltr">{children}</bdi>;
}

/**
 * Text whose direction is that of its own first strong character: a name a person typed (a saved
 * connection's name, a folder name), kept apart from the text around it.
 */
export function Own({ children }: { children: ReactNode }) {
	return <bdi>{children}</bdi>;
}

const LRI = '⁦';
const PDI = '⁩';

/**
 * `text` wrapped in Unicode isolates when the window is right to left, for a value put inside a
 * translated sentence or a title that is only a string (`tf('connect.hostKey.title', { host })`).
 * Left to right, it is the text itself.
 */
export function isolateLtr(text: string): string {
	if (text === '' || globalThis.document?.documentElement.dir !== 'rtl') return text;
	return `${LRI}${text}${PDI}`;
}

const FSI = '\u2068';

/** Like `isolateLtr`, for a name a person typed: it keeps the direction of its own first strong character. */
export function isolateOwn(text: string): string {
	if (text === '' || globalThis.document?.documentElement.dir !== 'rtl') return text;
	return `${FSI}${text}${PDI}`;
}
