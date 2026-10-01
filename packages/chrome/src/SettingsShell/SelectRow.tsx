// Settings row with a drop-down choice
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import '../tokens.css';
import { SettingsRow, useSettingsRowControl, type SettingsRowProps } from './SettingsRow';
import styles from './SettingsRow.module.css';

export interface SelectOption<T extends string> {
	value: T;
	label: string;
	disabled?: boolean;
}

export interface SelectRowProps<T extends string> extends Omit<SettingsRowProps, 'children'> {
	value: T;
	options: SelectOption<T>[];
	onChange: (value: T) => void;
}

function Select<T extends string>({
	value,
	options,
	onChange,
}: Pick<SelectRowProps<T>, 'value' | 'options' | 'onChange'>) {
	const { controlId, describedBy, disabled, invalid } = useSettingsRowControl();
	return (
		<select
			id={controlId}
			aria-describedby={describedBy}
			aria-invalid={invalid || undefined}
			disabled={disabled}
			className={styles.field}
			value={value}
			onChange={(event) => onChange(event.target.value as T)}
		>
			{options.map((option) => (
				<option key={option.value} value={option.value} disabled={option.disabled}>
					{option.label}
				</option>
			))}
		</select>
	);
}

export function SelectRow<T extends string>({
	value,
	options,
	onChange,
	...row
}: SelectRowProps<T>) {
	return (
		<SettingsRow {...row}>
			<Select value={value} options={options} onChange={onChange} />
		</SettingsRow>
	);
}
