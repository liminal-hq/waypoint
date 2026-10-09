// Which locations are elevated, and the same folder in its elevated and ordinary forms
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';

/** The URI scheme of an elevated location: `admin:///etc` is `/etc` as the elevated helper sees it. */
export const ELEVATED_SCHEME = 'admin';

const ELEVATED_PREFIX = `${ELEVATED_SCHEME}:`;
const FILE_PREFIX = 'file:';

/** Whether `uri` is written in the elevated scheme, in any letter case. */
export function isElevatedUri(uri: string): boolean {
	return uri.slice(0, ELEVATED_PREFIX.length).toLowerCase() === ELEVATED_PREFIX;
}

/** Whether `location` is elevated: the tab's location says so, so nothing else has to. */
export function isElevatedLocation(location: Pick<Location, 'uri'> | null | undefined): boolean {
	return location !== null && location !== undefined && isElevatedUri(location.uri);
}

/** Whether `uri` is an ordinary local `file:` one, which is what can be opened as an administrator. */
export function isFileUri(uri: string): boolean {
	return uri.slice(0, FILE_PREFIX.length).toLowerCase() === FILE_PREFIX;
}

/**
 * The elevated form of a `file:` URI, as `ElevatedPath::to_uri` writes it: the same text with
 * the elevated scheme. Rejects any other scheme.
 */
export function toElevatedUri(fileUri: string): string {
	if (!isFileUri(fileUri)) throw new RangeError(`not a file location: ${fileUri}`);
	return ELEVATED_PREFIX + fileUri.slice(FILE_PREFIX.length);
}

/**
 * The ordinary form of an elevated URI, as `ElevatedPath::from_uri` then `file()` reads it: the
 * same text with the `file:` scheme. Rejects any other scheme.
 */
export function toFileUri(elevatedUri: string): string {
	if (!isElevatedUri(elevatedUri)) throw new RangeError(`not an elevated location: ${elevatedUri}`);
	return FILE_PREFIX + elevatedUri.slice(ELEVATED_PREFIX.length);
}

/** The elevated location of a local folder. The display is the path itself: the mark is the tab's, not part of the name. */
export function elevatedLocation(location: Location): Location {
	return { display: location.display, uri: toElevatedUri(location.uri) };
}

/** The ordinary location of an elevated one: where leaving Administrator Mode goes. */
export function unelevatedLocation(location: Location): Location {
	return { display: location.display, uri: toFileUri(location.uri) };
}

/** What a context menu offers about Administrator Mode for the listing it was opened on. */
export interface ElevationMenu {
	/** Open as Administrator is offered: it is on and works here, and the listing is an ordinary local folder. */
	offer: boolean;
	/** The listing is shown as an administrator: Leave Administrator Mode is offered, and what cannot be handed to another program is hidden. */
	elevated: boolean;
}

/** The menu's view of `location` given whether administrator access is `offered` in this window. */
export function elevationMenuFor(
	offered: boolean,
	location: Pick<Location, 'uri'> | null | undefined,
): ElevationMenu {
	return {
		offer: offered && location !== null && location !== undefined && isFileUri(location.uri),
		elevated: isElevatedLocation(location),
	};
}
