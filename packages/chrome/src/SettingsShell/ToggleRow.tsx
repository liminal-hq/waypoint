// Settings row with an on/off switch
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import '../tokens.css';
import { SettingsRow, useSettingsRowControl, type SettingsRowProps } from './SettingsRow';
import styles from './SettingsRow.module.css';

export interface ToggleRowProps extends Omit<SettingsRowProps, 'children'> {
	checked: boolean;
	onChange: (checked: boolean) => void;
}

function Switch({ checked, onChange }: Pick<ToggleRowProps, 'checked' | 'onChange'>) {
	const { controlId, labelId, describedBy, disabled } = useSettingsRowControl();
	return (
		<button
			type="button"
			role="switch"
			id={controlId}
			aria-checked={checked}
			aria-labelledby={labelId}
			aria-describedby={describedBy}
			disabled={disabled}
			className={styles.switch}
			onClick={() => onChange(!checked)}
		>
			<span className={styles.thumb} aria-hidden="true" />
		</button>
	);
}

export function ToggleRow({ checked, onChange, ...row }: ToggleRowProps) {
	return (
		<SettingsRow {...row}>
			<Switch checked={checked} onChange={onChange} />
		</SettingsRow>
	);
}
