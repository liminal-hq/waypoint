// A small filled circle in a tab colour, for the Colour submenu and anywhere else a colour is named
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { TabColour } from '@liminal-hq/waypoint-protocol/generated/TabColour';
import styles from './TabColourSwatch.module.css';

/** Decorative: the colour's name is always shown or announced beside it. */
export function TabColourSwatch({ colour }: { colour: TabColour }) {
	return <span className={styles.swatch} data-colour={colour} aria-hidden="true" />;
}
