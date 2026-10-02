// Settings row with a single-line text field that applies when it is left or Enter is pressed
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useState } from 'react';
import '../tokens.css';
import { SettingsRow, useSettingsRowControl, type SettingsRowProps } from './SettingsRow';
import styles from './SettingsRow.module.css';

export interface TextRowProps extends Omit<SettingsRowProps, 'children'> {
	/** The value in force. The field shows what is typed until it is applied, then this again. */
	value: string;
	/** Applied when the field is left or Enter is pressed, and only when the text changed. */
	onCommit: (value: string) => void;
	/** Shown while the field is empty, such as the default. */
	placeholder?: string;
	/** The widest the field is, in characters. */
	size?: number;
}

function TextField({
	value,
	onCommit,
	placeholder,
	size = 20,
}: Omit<TextRowProps, keyof SettingsRowProps>) {
	const { controlId, labelId, describedBy, disabled, invalid } = useSettingsRowControl();
	// While typing the text is a draft; it is applied once, when typing ends.
	const [draft, setDraft] = useState<string | null>(null);
	const finish = () => {
		if (draft !== null && draft !== value) onCommit(draft);
		setDraft(null);
	};
	return (
		<input
			type="text"
			id={controlId}
			aria-labelledby={labelId}
			aria-describedby={describedBy}
			aria-invalid={invalid || undefined}
			disabled={disabled}
			className={styles.field}
			style={{ inlineSize: `${size}ch` }}
			value={draft ?? value}
			placeholder={placeholder}
			autoComplete="off"
			spellCheck={false}
			onChange={(event) => setDraft(event.target.value)}
			onBlur={finish}
			onKeyDown={(event) => {
				if (event.key === 'Enter') finish();
				if (event.key === 'Escape') setDraft(null);
			}}
		/>
	);
}

export function TextRow({ value, onCommit, placeholder, size, ...row }: TextRowProps) {
	return (
		<SettingsRow {...row}>
			<TextField value={value} onCommit={onCommit} placeholder={placeholder} size={size} />
		</SettingsRow>
	);
}
