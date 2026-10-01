// Settings row with a small set of mutually exclusive choices shown side by side
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useRef, type KeyboardEvent } from 'react';
import '../tokens.css';
import { SettingsRow, useSettingsRowControl, type SettingsRowProps } from './SettingsRow';
import styles from './SettingsRow.module.css';

export interface SegmentedOption<T extends string> {
	value: T;
	label: string;
	disabled?: boolean;
}

export interface SegmentedRowProps<T extends string> extends Omit<SettingsRowProps, 'children'> {
	value: T;
	options: SegmentedOption<T>[];
	onChange: (value: T) => void;
}

function Segments<T extends string>({
	value,
	options,
	onChange,
}: Pick<SegmentedRowProps<T>, 'value' | 'options' | 'onChange'>) {
	const { controlId, labelId, describedBy, disabled } = useSettingsRowControl();
	const group = useRef<HTMLDivElement>(null);
	const enabled = options.filter((option) => !option.disabled);
	const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
		const rtl = group.current ? getComputedStyle(group.current).direction === 'rtl' : false;
		const forward = rtl ? 'ArrowLeft' : 'ArrowRight';
		const back = rtl ? 'ArrowRight' : 'ArrowLeft';
		const current = enabled.findIndex((option) => option.value === value);
		let next = -1;
		if (event.key === forward || event.key === 'ArrowDown') next = (current + 1) % enabled.length;
		else if (event.key === back || event.key === 'ArrowUp')
			next = (current - 1 + enabled.length) % enabled.length;
		else if (event.key === 'Home') next = 0;
		else if (event.key === 'End') next = enabled.length - 1;
		const target = enabled[next];
		if (next < 0 || !target) return;
		event.preventDefault();
		onChange(target.value);
		group.current
			?.querySelector<HTMLElement>(`[data-value="${CSS.escape(target.value)}"]`)
			?.focus();
	};
	return (
		<div
			ref={group}
			id={controlId}
			role="radiogroup"
			aria-labelledby={labelId}
			aria-describedby={describedBy}
			aria-disabled={disabled || undefined}
			className={styles.segments}
			onKeyDown={onKeyDown}
		>
			{options.map((option) => {
				const checked = option.value === value;
				return (
					<button
						key={option.value}
						type="button"
						role="radio"
						aria-checked={checked}
						data-value={option.value}
						tabIndex={checked ? 0 : -1}
						disabled={disabled || option.disabled}
						className={`${styles.segment} ${checked ? styles.segmentOn : ''}`}
						onClick={() => onChange(option.value)}
					>
						{option.label}
					</button>
				);
			})}
		</div>
	);
}

export function SegmentedRow<T extends string>({
	value,
	options,
	onChange,
	...row
}: SegmentedRowProps<T>) {
	return (
		<SettingsRow {...row}>
			<Segments value={value} options={options} onChange={onChange} />
		</SettingsRow>
	);
}
