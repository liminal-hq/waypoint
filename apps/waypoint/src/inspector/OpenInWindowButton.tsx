// The Properties tab's "Open in a window": the same subject, in a Properties window of its own
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useOptionalVfsClient } from '../browse/VfsClientContext';
import { t } from '../i18n/messages';
import { openPropertiesWindowFor } from './openPropertiesWindow';
import { usePropertiesWindowClient } from './PropertiesWindowContext';
import type { InspectorSubject } from './useInspectorSubject';
import styles from './OpenInWindowButton.module.css';

/**
 * Offered for one entry or the folder, where the window has a Properties window service. Several
 * selected items are not one subject, and the Trash's items have no details to read.
 */
export function OpenInWindowButton({ subject }: { subject: InspectorSubject }) {
	const client = usePropertiesWindowClient();
	const vfs = useOptionalVfsClient();
	if (!client || !vfs || (subject.kind !== 'entry' && subject.kind !== 'folder')) return null;
	const open = async () => {
		try {
			const location =
				subject.kind === 'folder'
					? subject.location
					: await vfs.entryLocation(subject.handle, subject.entry.id);
			await openPropertiesWindowFor(client, location);
		} catch (error) {
			console.warn('could not name the Properties subject', error);
		}
	};
	return (
		<button type="button" className={styles.button} onClick={() => void open()}>
			{t('properties.open')}
		</button>
	);
}
