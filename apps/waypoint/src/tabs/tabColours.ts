// The tab colour palette: the wire type's names in menu order, with their catalogue messages
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { TabColour } from '@liminal-hq/waypoint-protocol/generated/TabColour';
import type { MessageId } from '../i18n/messages';

/** Every colour a tab can carry, in the order the Colour submenu lists them. */
export const TAB_COLOURS: readonly TabColour[] = [
	'red',
	'orange',
	'yellow',
	'green',
	'teal',
	'blue',
	'purple',
	'pink',
	'grey',
];

const COLOUR_MESSAGES: Record<TabColour, MessageId> = {
	red: 'tabs.colour.red',
	orange: 'tabs.colour.orange',
	yellow: 'tabs.colour.yellow',
	green: 'tabs.colour.green',
	teal: 'tabs.colour.teal',
	blue: 'tabs.colour.blue',
	purple: 'tabs.colour.purple',
	pink: 'tabs.colour.pink',
	grey: 'tabs.colour.grey',
};

/** The catalogue message that names `colour`. */
export function colourMessageId(colour: TabColour): MessageId {
	return COLOUR_MESSAGES[colour];
}
