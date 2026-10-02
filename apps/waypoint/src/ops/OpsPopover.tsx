// The queue popover: a non-modal panel anchored above the ring, closed with Escape or a click elsewhere
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useRef, type RefObject } from 'react';
import { t } from '../i18n/messages';
import styles from './OpsIndicator.module.css';
import { OpsPanel } from './OpsPanel';

interface OpsPopoverProps {
	id: string;
	/** The element that contains the trigger and this popover: a press inside it is not "outside". */
	boundary: RefObject<HTMLElement | null>;
	/** `escape` returns focus to the ring; the others leave it where the person put it. */
	onClose: (reason: 'escape' | 'outside' | 'action') => void;
}

/**
 * Not modal: the rest of the window stays usable and Tab moves on through it. Focus goes to the
 * first row when it opens (or to the panel when the list is empty), Escape closes it and gives focus
 * back to the ring, and a press or focus outside closes it without taking focus.
 */
export function OpsPopover({ id, boundary, onClose }: OpsPopoverProps) {
	const close = useRef(onClose);
	close.current = onClose;

	useEffect(() => {
		const outside = (event: PointerEvent) => {
			if (boundary.current?.contains(event.target as Node | null)) return;
			close.current('outside');
		};
		document.addEventListener('pointerdown', outside, true);
		return () => document.removeEventListener('pointerdown', outside, true);
	}, [boundary]);

	return (
		<div
			id={id}
			className={styles.popover}
			role="dialog"
			aria-modal="false"
			aria-label={t('ops.popover.label')}
			onKeyDown={(event) => {
				if (event.key !== 'Escape') return;
				event.preventDefault();
				event.stopPropagation();
				close.current('escape');
			}}
			onBlur={(event) => {
				const next = event.relatedTarget as Node | null;
				if (next && !boundary.current?.contains(next)) close.current('outside');
			}}
		>
			<OpsPanel layout="popover" autoFocus onDone={() => close.current('action')} />
		</div>
	);
}
