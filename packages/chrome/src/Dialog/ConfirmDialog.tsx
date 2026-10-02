// Confirmation dialog with a safe default focus
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ReactNode } from 'react';
import { Dialog, type DialogSize } from './Dialog';
import { DialogActions, DialogButton } from './DialogActions';

export interface ConfirmDialogProps {
	open: boolean;
	title: ReactNode;
	message: ReactNode;
	confirmLabel: string;
	cancelLabel?: string;
	/** Styles the confirm button as destructive; focus then starts on Cancel. */
	danger?: boolean;
	size?: DialogSize;
	onConfirm: () => void;
	/** Cancel, Esc and a backdrop click all land here. */
	onCancel: () => void;
}

export function ConfirmDialog({
	open,
	title,
	message,
	confirmLabel,
	cancelLabel = 'Cancel',
	danger = false,
	size = 'small',
	onConfirm,
	onCancel,
}: ConfirmDialogProps) {
	return (
		<Dialog
			open={open}
			title={title}
			description={message}
			size={size}
			onClose={() => onCancel()}
			footer={
				<DialogActions>
					<DialogButton variant="secondary" onClick={onCancel}>
						{cancelLabel}
					</DialogButton>
					<DialogButton variant={danger ? 'danger' : 'primary'} onClick={onConfirm}>
						{confirmLabel}
					</DialogButton>
				</DialogActions>
			}
		/>
	);
}
