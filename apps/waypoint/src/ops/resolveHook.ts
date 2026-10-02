// Where the queue UI hands a waiting job to whatever answers it (the conflict and error dialogs)
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { showNotice } from '../app/notices';
import { t } from '../i18n/messages';

/** Opens the dialog that answers `job`, which is waiting on conflicts or on an error. */
export type Resolver = (job: number) => void;

let resolver: Resolver | null = null;

/**
 * Registers the function that answers a waiting job; the conflict and error dialogs do. Returns what
 * removes it again. A later registration replaces an earlier one.
 */
export function registerResolver(next: Resolver): () => void {
	resolver = next;
	return () => {
		if (resolver === next) resolver = null;
	};
}

/**
 * "Resolve…" on a waiting row. Until a dialog is registered it says so, instead of leaving a button
 * that does nothing.
 */
export function requestResolve(job: number): void {
	if (resolver) resolver(job);
	else showNotice(t('ops.resolve.unavailable'));
}
