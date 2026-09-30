// Window frame: clips a frameless window to rounded corners, square when maximised
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ReactNode } from 'react';
import { useWindowFocused, useWindowMaximised } from '../WindowChromeProvider/WindowChromeProvider';
import '../tokens.css';
import styles from './WindowFrame.module.css';

export interface WindowFrameProps {
	children: ReactNode;
	className?: string;
}

/**
 * The outermost element of a frameless, transparent window. Its surface is rounded
 * and clipped to `--wp-window-radius`, given a hairline border, and shadowed with
 * `--wp-window-shadow` inside a transparent margin of `--wp-window-shadow-margin`.
 * While the window is unfocused it uses `--wp-window-shadow-unfocused` instead, falling
 * back to the focused shadow. The margin and shadow default to none, for platforms where
 * the OS draws its own.
 * When the window is maximised the margin, shadow, rounding and border all drop so
 * it meets the screen edges cleanly. `className` applies to the visible surface. Window state comes
 * from `WindowChromeProvider`, which must be an ancestor.
 */
export function WindowFrame({ children, className }: WindowFrameProps) {
	const maximised = useWindowMaximised();
	const focused = useWindowFocused();
	const surface = [styles.surface, className ?? ''].filter(Boolean).join(' ');
	return (
		<div className={styles.frame} data-maximised={maximised} data-focused={focused}>
			<div className={surface}>{children}</div>
		</div>
	);
}
