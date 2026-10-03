// A shortcut shown as a keyboard chip
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import styles from './Help.module.css';

/** The registry's display form of a key ("Ctrl+Shift+Z") as one `<kbd>`; the caller passes only keys that exist. */
export function KeyChips({ keys }: { keys: string }) {
	return (
		<span className={styles.keys}>
			<kbd className={styles.key}>{keys}</kbd>
		</span>
	);
}
