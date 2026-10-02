// Settings row with a bounded number field and an optional unit
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useState } from 'react';
import '../tokens.css';
import { SettingsRow, useSettingsRowControl, type SettingsRowProps } from './SettingsRow';
import styles from './SettingsRow.module.css';

export interface NumberRowProps extends Omit<SettingsRowProps, 'children'> {
	value: number;
	onChange: (value: number) => void;
	min?: number;
	max?: number;
	step?: number;
	/** Suffix shown after the field, such as "MB" or "ms". */
	unit?: string;
	/**
	 * When a typed value is applied: on every valid keystroke (`change`, the default), or only
	 * when the field is left or Enter is pressed (`commit`), for a setting that must not take the
	 * half-typed values on the way to the one meant.
	 */
	commitOn?: 'change' | 'commit';
}

function NumberField({
	value,
	onChange,
	min,
	max,
	step,
	unit,
	commitOn = 'change',
}: Omit<NumberRowProps, keyof SettingsRowProps>) {
	const { controlId, labelId, describedBy, disabled, invalid } = useSettingsRowControl();
	// While typing the text may be empty or out of range; the value only changes for valid numbers.
	const [draft, setDraft] = useState<string | null>(null);
	const inRange = (n: number) => (min === undefined || n >= min) && (max === undefined || n <= max);
	const clamp = (n: number) => Math.min(max ?? n, Math.max(min ?? n, n));
	const finish = () => {
		if (draft !== null) {
			const n = Number(draft);
			if (draft.trim() !== '' && Number.isFinite(n)) {
				const next = clamp(n);
				if (next !== value) onChange(next);
			}
		}
		setDraft(null);
	};
	return (
		<span className={styles.numberWrap}>
			<input
				type="number"
				id={controlId}
				aria-labelledby={labelId}
				aria-describedby={describedBy}
				aria-invalid={invalid || undefined}
				disabled={disabled}
				className={`${styles.field} ${styles.number}`}
				value={draft ?? String(value)}
				min={min}
				max={max}
				step={step}
				onChange={(event) => {
					setDraft(event.target.value);
					const n = Number(event.target.value);
					if (
						commitOn === 'change' &&
						event.target.value.trim() !== '' &&
						Number.isFinite(n) &&
						inRange(n)
					)
						onChange(n);
				}}
				onBlur={finish}
				onKeyDown={(event) => {
					if (event.key === 'Enter') finish();
				}}
			/>
			{unit ? (
				<span className={styles.unit} aria-hidden="true">
					{unit}
				</span>
			) : null}
		</span>
	);
}

export function NumberRow({
	value,
	onChange,
	min,
	max,
	step,
	unit,
	commitOn,
	...row
}: NumberRowProps) {
	return (
		<SettingsRow {...row}>
			<NumberField
				value={value}
				onChange={onChange}
				min={min}
				max={max}
				step={step}
				unit={unit}
				commitOn={commitOn}
			/>
		</SettingsRow>
	);
}
