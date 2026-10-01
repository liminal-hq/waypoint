// The toast for a notice: a polite status message with an action button, dismissed after a while
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useRef, useState } from 'react';
import { t } from '../i18n/messages';
import { CloseSmallIcon } from '../icons/AppIcons';
import { dismissNotice, useNotice } from './notices';
import styles from './NoticeToast.module.css';

/** How long a notice stays up when nobody is pointing at it or focused in it. */
export const NOTICE_TOAST_MS = 8000;

/**
 * The live region is always mounted (and empty when there is no notice), so a screen reader
 * announces the message when it arrives. The action is a real button, reached with Tab, and
 * Escape dismisses the toast when focus is inside it. The timer waits while the pointer or focus
 * is on the toast.
 */
export function NoticeToast() {
	const notice = useNotice();
	const [held, setHeld] = useState(false);
	const toast = useRef<HTMLDivElement | null>(null);
	const id = notice?.id;

	useEffect(() => {
		setHeld(false);
	}, [id]);

	useEffect(() => {
		if (id === undefined || held) return;
		const timer = setTimeout(() => dismissNotice(id), NOTICE_TOAST_MS);
		return () => clearTimeout(timer);
	}, [id, held]);

	return (
		<div className={styles.region} role="status" aria-live="polite">
			{notice ? (
				<div
					ref={toast}
					className={styles.toast}
					data-notice=""
					onPointerEnter={() => setHeld(true)}
					onPointerLeave={() => setHeld(toast.current?.contains(document.activeElement) ?? false)}
					onFocus={() => setHeld(true)}
					onBlur={(event) => {
						if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setHeld(false);
					}}
					onKeyDown={(event) => {
						if (event.key !== 'Escape') return;
						event.preventDefault();
						dismissNotice(notice.id);
					}}
				>
					<span className={styles.text}>{notice.text}</span>
					{notice.action ? (
						<button
							type="button"
							className={styles.action}
							onClick={() => {
								notice.action?.run();
								dismissNotice(notice.id);
							}}
						>
							{notice.action.label}
						</button>
					) : null}
					<button
						type="button"
						className={styles.dismiss}
						aria-label={t('notice.dismiss')}
						title={t('notice.dismiss')}
						onClick={() => dismissNotice(notice.id)}
					>
						<CloseSmallIcon width={12} height={12} />
					</button>
				</div>
			) : null}
		</div>
	);
}
