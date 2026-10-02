// What the Trash place shows where the Trash cannot be browsed: the reason, never a blank view
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { MessageState } from '../browse/ListingGate';
import listingGate from '../browse/ListingGate.module.css';
import { t } from '../i18n/messages';

/** The system's own reason (a sandbox's portal can only move files to the Trash) under the plain statement. */
export function TrashUnavailable({ reason }: { reason: string | null }) {
	return (
		<MessageState role="status" data-state="trash-unavailable">
			<h2 className={listingGate.messageTitle}>{t('trash.unavailable.title')}</h2>
			<p className={listingGate.messageDetail}>{reason ?? t('trash.unavailable.fallback')}</p>
		</MessageState>
	);
}
