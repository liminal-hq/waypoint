// An archive that is locked: says so, and asks for the password in the same dialog a login uses
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useState } from 'react';
import { askQuestion } from '../connections/connectStore';
import { useRetryRemote } from '../connections/RemoteState';
import styles from '../connections/RemoteState.module.css';
import { t, tf } from '../i18n/messages';
import { announce } from '../tabs/announcer';
import { commandErrorMessage } from './askPassphrase';
import { useArchiveClient } from './ArchiveContext';
import { lockedName, type ArchiveLock } from './lockModel';

/**
 * The state of an archive whose contents need a password (D152, D162): the title and a reason in
 * words, and one action, Enter Password, which asks in the dialog a login asks in and gives the
 * password to Rust. Once it is given, the folders on screen that failed open again; a wrong
 * password comes back here, saying so.
 */
export function ArchiveLocked({ error }: { error: ArchiveLock }) {
	const archives = useArchiveClient();
	const retry = useRetryRemote();
	const [busy, setBusy] = useState(false);
	const [problem, setProblem] = useState<string | null>(null);
	const name = lockedName(error.location);

	const enter = async () => {
		if (!archives || busy) return;
		setBusy(true);
		setProblem(null);
		try {
			const answered = await askQuestion({
				kind: 'authRequired',
				location: error.location,
				prompt: { kind: 'passphrase', subject: name },
			});
			if (answered?.answer.kind !== 'passphrase') return;
			await archives.unlock(error.location, answered.answer.passphrase);
			announce(tf('archive.locked.announce', { name }));
			retry();
		} catch (failure) {
			const words = commandErrorMessage(failure);
			setProblem(words);
			announce(words);
		} finally {
			setBusy(false);
		}
	};

	return (
		<div className={styles.state} role="alert" data-error={error.kind} data-archive-locked="">
			<h2 className={styles.title}>{tf('archive.locked.title', { name })}</h2>
			<p className={styles.detail}>
				{t(error.kind === 'authFailed' ? 'archive.locked.refused' : 'archive.locked.detail')}
			</p>
			{archives && (
				<button
					type="button"
					className={styles.action}
					disabled={busy}
					onClick={() => void enter()}
				>
					{busy ? t('archive.locked.checking') : t('archive.locked.action')}
				</button>
			)}
			{problem && <p className={styles.problem}>{problem}</p>}
		</div>
	);
}
