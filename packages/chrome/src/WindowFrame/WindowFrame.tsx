// Window frame: clips a frameless window to rounded corners, square when maximised
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ReactNode } from 'react';
import { useMaximised } from '../TitleBar/useMaximised';
import type { WindowControls } from '../TitleBar/windowControls';
import styles from './WindowFrame.module.css';

export interface WindowFrameProps {
	windowControls: Pick<WindowControls, 'isMaximized' | 'onMaximizedChange'>;
	children: ReactNode;
	className?: string;
}

/**
 * The outermost element of a frameless, transparent window. Its surface is rounded
 * and clipped to `--wp-window-radius`, given a hairline border, and shadowed with
 * `--wp-window-shadow` inside a transparent margin of `--wp-window-shadow-margin`.
 * The margin and shadow default to none, for platforms where the OS draws its own.
 * When the window is maximised the margin, shadow, rounding and border all drop so
 * it meets the screen edges cleanly. `className` applies to the visible surface.
 */
export function WindowFrame({ windowControls, children, className }: WindowFrameProps) {
	const maximised = useMaximised(windowControls);
	const surface = [styles.surface, className ?? ''].filter(Boolean).join(' ');
	return (
		<div className={styles.frame} data-maximised={maximised}>
			<div className={surface}>{children}</div>
		</div>
	);
}
