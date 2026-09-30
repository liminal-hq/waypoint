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
 * The outermost element of a frameless, transparent window. It rounds and clips
 * everything inside it to `--wp-window-radius`, draws a hairline border, and drops
 * both when the window is maximised so it meets the screen edges cleanly.
 */
export function WindowFrame({ windowControls, children, className }: WindowFrameProps) {
	const maximised = useMaximised(windowControls);
	const classes = [styles.frame, className ?? ''].filter(Boolean).join(' ');
	return (
		<div className={classes} data-maximised={maximised}>
			{children}
		</div>
	);
}
