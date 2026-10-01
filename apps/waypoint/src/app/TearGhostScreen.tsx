// The tear-off ghost: a static card that draws whatever payload the drag sends it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { getPayload, onPayload } from '@liminal-hq/plugin-window-tearoff';
import { useEffect, useState } from 'react';
import styles from './TearGhostScreen.module.css';

/** What a drag sends the ghost; every field is optional so a partial payload still draws. */
export interface GhostPayload {
	title?: string;
	count?: number;
	label?: string;
}

/** Narrows an opaque payload to the fields the card draws, dropping anything of the wrong type. */
export function readGhostPayload(raw: unknown): GhostPayload | null {
	if (typeof raw !== 'object' || raw === null) return null;
	const { title, count, label } = raw as Record<string, unknown>;
	const payload: GhostPayload = {};
	if (typeof title === 'string') payload.title = title;
	if (typeof count === 'number' && Number.isFinite(count)) payload.count = count;
	if (typeof label === 'string') payload.label = label;
	return Object.keys(payload).length > 0 ? payload : null;
}

/**
 * The card is drawn only while a drag has sent a payload, so the pre-created window stays empty and transparent between drags.
 * It reads the current payload once on mount (the page may load after the drag began) and then follows the payload events.
 * The plugin sends a `null` payload when a drag ends, which clears the card before the window is next shown.
 */
export function TearGhostScreen() {
	const [payload, setPayload] = useState<GhostPayload | null>(null);

	useEffect(() => {
		let active = true;
		let unlisten: (() => void) | undefined;
		void onPayload((raw) => {
			if (active) setPayload(readGhostPayload(raw));
		}).then((stop) => {
			if (active) unlisten = stop;
			else stop();
		});
		void getPayload()
			.then((raw) => {
				if (active) setPayload((current) => current ?? readGhostPayload(raw));
			})
			.catch(() => {});
		return () => {
			active = false;
			unlisten?.();
		};
	}, []);

	if (!payload) return null;

	return (
		<div className={styles.card} data-testid="tear-ghost-card">
			{payload.count !== undefined && <span className={styles.count}>{payload.count}</span>}
			<div className={styles.text}>
				{payload.title !== undefined && <span className={styles.title}>{payload.title}</span>}
				{payload.label !== undefined && <span className={styles.label}>{payload.label}</span>}
			</div>
		</div>
	);
}
