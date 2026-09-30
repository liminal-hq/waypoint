// Draggable, truncating title text for the title bar's centre slot
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ReactNode } from 'react';
import styles from './TitleBarTitle.module.css';

export interface TitleBarTitleProps {
	children: ReactNode;
	className?: string;
}

/**
 * Static title text meant for the `center` slot. Tauri's drag handler checks only the pressed
 * element, not its ancestors, so the text carries `data-tauri-drag-region` itself. It truncates
 * to a single line with an ellipsis.
 */
export function TitleBarTitle({ children, className }: TitleBarTitleProps) {
	const classes = [styles.title, className ?? ''].filter(Boolean).join(' ');
	return (
		<span className={classes} data-tauri-drag-region="">
			{children}
		</span>
	);
}
