// The Unlock dialog: a passphrase for an encrypted volume, asked for once and never kept
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Volume } from '@liminal-hq/plugin-volumes';
import { Dialog } from '@liminal-hq/waypoint-chrome/Dialog/Dialog';
import { DialogActions, DialogButton } from '@liminal-hq/waypoint-chrome/Dialog/DialogActions';
import { useId, useState, type KeyboardEvent } from 'react';
import { t, tf } from '../i18n/messages';
import type { UnlockResult } from './useDevices';
import styles from './DeviceList.module.css';

interface UnlockDialogProps {
	volume: Volume;
	/** Tries the passphrase; the dialog stays open on `wrong` and asks again. */
	onUnlock(volume: Volume, passphrase: string): Promise<UnlockResult>;
	onClose(): void;
}

/**
 * The field has focus when it opens; Enter unlocks and Esc cancels. There is no "remember this
 * passphrase" (D120): the text lives in this component's state and goes with it when the dialog
 * closes. A wrong passphrase is said in words under the field and the field is selected again.
 */
export function UnlockDialog({ volume, onUnlock, onClose }: UnlockDialogProps) {
	const [passphrase, setPassphrase] = useState('');
	const [wrong, setWrong] = useState(false);
	const [working, setWorking] = useState(false);
	const fieldId = useId();
	const problemId = useId();

	const submit = async () => {
		if (working || passphrase === '') return;
		setWorking(true);
		setWrong(false);
		const result = await onUnlock(volume, passphrase);
		if (result === 'wrong') {
			setWorking(false);
			setWrong(true);
			setPassphrase('');
			document.getElementById(fieldId)?.focus();
			return;
		}
		onClose();
	};

	const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
		if (event.key !== 'Enter') return;
		event.preventDefault();
		void submit();
	};

	return (
		<Dialog
			open
			size="small"
			title={tf('devices.unlock.title', { name: volume.label })}
			description={t('devices.unlock.description')}
			initialFocus={`#${CSS.escape(fieldId)}`}
			onClose={() => {
				if (!working) onClose();
			}}
			footer={
				<DialogActions>
					<DialogButton variant="secondary" disabled={working} onClick={onClose}>
						{t('devices.unlock.cancel')}
					</DialogButton>
					<DialogButton
						variant="primary"
						disabled={working || passphrase === ''}
						onClick={() => void submit()}
					>
						{working ? t('devices.unlock.working') : t('devices.unlock.confirm')}
					</DialogButton>
				</DialogActions>
			}
		>
			<div className={styles.field}>
				<label htmlFor={fieldId} className={styles.label}>
					{t('devices.unlock.field')}
				</label>
				<input
					id={fieldId}
					type="password"
					className={styles.input}
					value={passphrase}
					autoComplete="off"
					spellCheck={false}
					disabled={working}
					aria-invalid={wrong || undefined}
					aria-describedby={problemId}
					onChange={(event) => setPassphrase(event.target.value)}
					onKeyDown={onKeyDown}
				/>
				<p id={problemId} className={styles.problem} role="alert">
					{wrong ? t('devices.unlock.wrong') : ''}
				</p>
			</div>
		</Dialog>
	);
}
