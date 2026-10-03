// The Help dialog: a few keys worth knowing, and the way to the shortcut list, the tour and About
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Dialog } from '@liminal-hq/waypoint-chrome/Dialog/Dialog';
import { DialogActions, DialogButton } from '@liminal-hq/waypoint-chrome/Dialog/DialogActions';
import type { CommandView } from '../commands/registry';
import { t } from '../i18n/messages';
import { HelpArt } from './HelpArt';
import styles from './Help.module.css';
import { helpRows } from './helpModel';
import type { HelpPage } from './helpPages';
import { KeyChips } from './KeyChips';

interface HelpDialogProps {
	/** The window's commands, which say what to list and which key each has. */
	views: readonly CommandView[];
	onClose: () => void;
	onOpen: (page: HelpPage) => void;
}

/** Help opens with Close focused, so Enter never takes someone further into the help than they chose. */
export function HelpDialog({ views, onClose, onOpen }: HelpDialogProps) {
	const rows = helpRows(views);
	return (
		<Dialog
			open
			onClose={onClose}
			title={t('help.title')}
			description={t('help.subtitle')}
			initialFocus="[data-help-close]"
			footer={
				<DialogActions>
					<DialogButton data-help-close="" closes>
						{t('help.close')}
					</DialogButton>
					<DialogButton onClick={() => onOpen('shortcuts')}>
						{t('cmd.keyboardShortcuts')}
					</DialogButton>
					<DialogButton onClick={() => onOpen('tour')}>{t('cmd.tour')}</DialogButton>
					<DialogButton variant="primary" onClick={() => onOpen('about')}>
						{t('help.about')}
					</DialogButton>
				</DialogActions>
			}
		>
			<HelpArt />
			<ul className={styles.rows} aria-label={t('help.rows.label')}>
				{rows.map((row) => (
					<li key={row.id} className={styles.row}>
						<span className={styles.action}>{row.text}</span>
						<KeyChips keys={row.keys} />
					</li>
				))}
			</ul>
		</Dialog>
	);
}
