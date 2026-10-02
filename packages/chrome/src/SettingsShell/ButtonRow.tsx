// Settings row whose control is a single action button
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import '../tokens.css';
import { SettingsRow, useSettingsRowControl, type SettingsRowProps } from './SettingsRow';
import styles from './SettingsRow.module.css';

export interface ButtonRowProps extends Omit<SettingsRowProps, 'children' | 'associateLabel'> {
	/** The button's text, such as "Export logs…". */
	actionLabel: string;
	onAction: () => void;
	/** Styles the button as destructive, for actions such as resetting settings. */
	danger?: boolean;
}

function ActionButton({
	actionLabel,
	onAction,
	danger,
}: Pick<ButtonRowProps, 'actionLabel' | 'onAction' | 'danger'>) {
	const { controlId, labelId, describedBy, disabled } = useSettingsRowControl();
	return (
		<button
			type="button"
			id={controlId}
			// The visible text names the action; the row label and description follow as its description.
			aria-describedby={[labelId, describedBy].filter(Boolean).join(' ')}
			disabled={disabled}
			className={`${styles.button} ${danger ? styles.buttonDanger : ''}`}
			onClick={onAction}
		>
			{actionLabel}
		</button>
	);
}

export function ButtonRow({ actionLabel, onAction, danger, ...row }: ButtonRowProps) {
	return (
		<SettingsRow {...row} associateLabel={false}>
			<ActionButton actionLabel={actionLabel} onAction={onAction} danger={danger} />
		</SettingsRow>
	);
}
