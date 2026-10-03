// The Keyboard Shortcuts dialog: every offered command that has a key, grouped as the palette groups them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Dialog } from '@liminal-hq/waypoint-chrome/Dialog/Dialog';
import { DialogActions, DialogButton } from '@liminal-hq/waypoint-chrome/Dialog/DialogActions';
import type { CommandView } from '../commands/registry';
import { t, tf } from '../i18n/messages';
import styles from './Help.module.css';
import { shortcutGroups } from './helpModel';
import { KeyChips } from './KeyChips';

interface KeyboardShortcutsDialogProps {
	views: readonly CommandView[];
	onClose: () => void;
}

export function KeyboardShortcutsDialog({ views, onClose }: KeyboardShortcutsDialogProps) {
	const groups = shortcutGroups(views);
	return (
		<Dialog
			open
			onClose={onClose}
			title={t('shortcuts.title')}
			description={t('shortcuts.subtitle')}
			footer={
				<DialogActions>
					<DialogButton variant="primary" closes>
						{t('shortcuts.done')}
					</DialogButton>
				</DialogActions>
			}
		>
			{groups.length === 0 ? (
				<p className={styles.empty}>{t('shortcuts.empty')}</p>
			) : (
				groups.map((group) => (
					<section key={group.group} className={styles.group}>
						<h3 className={styles.groupTitle}>{group.label}</h3>
						<ul
							className={`${styles.rows} ${styles.plain}`}
							aria-label={tf('shortcuts.list.label', { group: group.label })}
						>
							{group.rows.map((row) => (
								<li key={row.id} className={styles.row}>
									<span className={styles.action}>{row.label}</span>
									<KeyChips keys={row.keys} />
								</li>
							))}
						</ul>
					</section>
				))
			)}
		</Dialog>
	);
}
