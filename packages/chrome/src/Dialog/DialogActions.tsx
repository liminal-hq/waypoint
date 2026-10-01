// Button row and button variants for a dialog footer
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ButtonHTMLAttributes, ReactNode } from 'react';
import '../tokens.css';
import styles from './DialogActions.module.css';
import { useDialogContext } from './dialogContext';

export function DialogActions({ children }: { children: ReactNode }) {
	return <div className={styles.actions}>{children}</div>;
}

export type DialogButtonVariant = 'primary' | 'secondary' | 'danger';

export interface DialogButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'type'> {
	variant?: DialogButtonVariant;
	/** Also asks the enclosing dialog to close with reason `'action'` after `onClick` runs. */
	closes?: boolean;
	type?: 'button' | 'submit';
}

/** A footer button. The dialog's default focus picks among these by variant, and never a danger one. */
export function DialogButton({
	variant = 'secondary',
	closes = false,
	onClick,
	type = 'button',
	className,
	...rest
}: DialogButtonProps) {
	const dialog = useDialogContext();
	return (
		<button
			{...rest}
			type={type}
			data-dialog-button=""
			data-variant={variant}
			className={[styles.button, styles[variant], className].filter(Boolean).join(' ')}
			onClick={(event) => {
				onClick?.(event);
				if (closes && !event.defaultPrevented) dialog?.requestClose('action');
			}}
		/>
	);
}
