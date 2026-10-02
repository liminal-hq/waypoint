// The Trash view's frame: its action strip over the list, with the list taking the keyboard when the view opens
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useRef, type KeyboardEventHandler, type ReactNode } from 'react';
import type { ListingSession } from '../browse/useListingSession';
import { TrashBar } from './TrashBar';
import styles from './TrashView.module.css';

interface TrashFrameProps {
	session: ListingSession;
	onKeyDown: KeyboardEventHandler<HTMLDivElement>;
	children: ReactNode;
}

/**
 * Opening the Trash from the sidebar or the palette leaves focus on the page body, where the arrow
 * keys and Ctrl+A do nothing useful, so the list takes focus when the view opens, unless focus is
 * already somewhere on purpose.
 */
export function TrashFrame({ session, onKeyDown, children }: TrashFrameProps) {
	const frame = useRef<HTMLDivElement | null>(null);
	useEffect(() => {
		const active = document.activeElement;
		if (active && active !== document.body) return;
		frame.current?.querySelector<HTMLElement>('[role="listbox"]')?.focus();
	}, [session]);
	return (
		<div ref={frame} className={styles.view} onKeyDown={onKeyDown} data-trash="">
			<TrashBar session={session} />
			<div className={styles.body}>{children}</div>
		</div>
	);
}
