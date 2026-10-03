// Settings row with a colour picker, for a value written as #rrggbb
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import '../tokens.css';
import { SettingsRow, useSettingsRowControl, type SettingsRowProps } from './SettingsRow';
import styles from './SettingsRow.module.css';

export interface ColourRowProps extends Omit<SettingsRowProps, 'children'> {
	/** `#rrggbb`. */
	value: string;
	onChange: (value: string) => void;
}

function Colour({ value, onChange }: Pick<ColourRowProps, 'value' | 'onChange'>) {
	const { controlId, describedBy, disabled, invalid } = useSettingsRowControl();
	return (
		<input
			id={controlId}
			type="color"
			aria-describedby={describedBy}
			aria-invalid={invalid || undefined}
			disabled={disabled}
			className={styles.field}
			value={value}
			onChange={(event) => onChange(event.target.value)}
		/>
	);
}

export function ColourRow({ value, onChange, ...row }: ColourRowProps) {
	return (
		<SettingsRow {...row}>
			<Colour value={value} onChange={onChange} />
		</SettingsRow>
	);
}
