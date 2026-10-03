// The About dialog: the name, the version the application reports, its licence and what it is built with
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Dialog } from '@liminal-hq/waypoint-chrome/Dialog/Dialog';
import { DialogActions, DialogButton } from '@liminal-hq/waypoint-chrome/Dialog/DialogActions';
import { useEffect, useState } from 'react';
import { t, tf } from '../i18n/messages';
import type { AppInfoClient } from '../services/appInfoClient';
import styles from './Help.module.css';

interface AboutDialogProps {
	/** Where the version comes from; without one it is reported as unreadable. */
	client?: AppInfoClient;
	onClose: () => void;
}

type Version = { state: 'loading' } | { state: 'ready'; version: string } | { state: 'failed' };

export function AboutDialog({ client, onClose }: AboutDialogProps) {
	const [version, setVersion] = useState<Version>(
		client ? { state: 'loading' } : { state: 'failed' },
	);
	useEffect(() => {
		if (!client) return;
		let live = true;
		client.getVersion().then(
			(value) => live && setVersion({ state: 'ready', version: value }),
			(error: unknown) => {
				console.warn('could not read the application version', error);
				if (live) setVersion({ state: 'failed' });
			},
		);
		return () => {
			live = false;
		};
	}, [client]);

	const subtitle =
		version.state === 'ready'
			? tf('about.version', { version: version.version })
			: t(version.state === 'loading' ? 'about.versionLoading' : 'about.versionUnavailable');
	return (
		<Dialog
			open
			onClose={onClose}
			title={t('about.title')}
			description={subtitle}
			footer={
				<DialogActions>
					<DialogButton variant="primary" closes>
						{t('about.close')}
					</DialogButton>
				</DialogActions>
			}
		>
			<dl className={styles.facts} aria-label={t('about.rows.label')}>
				<div className={styles.fact}>
					<dt>{t('about.licence')}</dt>
					<dd>{t('about.licence.value')}</dd>
				</div>
				<div className={styles.fact}>
					<dt>{t('about.builtWith')}</dt>
					<dd>{t('about.builtWith.value')}</dd>
				</div>
			</dl>
			<p className={styles.credit}>{t('about.credit')}</p>
		</Dialog>
	);
}
