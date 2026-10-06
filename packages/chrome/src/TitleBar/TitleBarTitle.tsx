// Draggable, truncating title text for the title bar's centre slot
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useSyncExternalStore, type ReactNode } from 'react';
import {
	useOptionalTitleFormat,
	useOptionalWindowTitleStore,
} from '../WindowChromeProvider/WindowChromeProvider';
import styles from './TitleBarTitle.module.css';

export interface TitleBarTitleProps {
	/** Fixed text. Without it the title follows `useWindowTitle`. */
	children?: ReactNode;
	/** Shown, as given, until something has set a title. */
	fallback?: ReactNode;
	className?: string;
}

const noSubscription = () => () => {};
const noTitle = () => undefined;

/**
 * Title text meant for the `center` slot. Given `children` it shows them as they are. Without, it
 * shows the title set through `useWindowTitle` (passed through the provider's `formatTitle`), and
 * `fallback` until one is set. Tauri's drag handler checks only the pressed element, not its
 * ancestors, so the text carries `data-tauri-drag-region` itself. It truncates to a single line
 * with an ellipsis.
 */
export function TitleBarTitle({ children, fallback, className }: TitleBarTitleProps) {
	const store = useOptionalWindowTitleStore();
	const format = useOptionalTitleFormat();
	const shared = useSyncExternalStore(store?.subscribe ?? noSubscription, store?.get ?? noTitle);
	const classes = [styles.title, className ?? ''].filter(Boolean).join(' ');
	const text = children ?? (shared === undefined ? fallback : format ? format(shared) : shared);
	return (
		<span className={classes} data-tauri-drag-region="">
			{text}
		</span>
	);
}
