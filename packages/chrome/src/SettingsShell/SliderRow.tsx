// Settings row with a slider that previews while it is dragged and applies when it is let go
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useRef, useState } from 'react';
import '../tokens.css';
import { SettingsRow, useSettingsRowControl, type SettingsRowProps } from './SettingsRow';
import styles from './SettingsRow.module.css';

export interface SliderRowProps extends Omit<SettingsRowProps, 'children'> {
	/** The value in force. */
	value: number;
	min: number;
	max: number;
	step?: number;
	/** Suffix shown after the value, such as "%". */
	unit?: string;
	/** Called with every value the slider passes through while it is dragged (and for each key press), for a preview that follows the thumb. */
	onInput?: (value: number) => void;
	/** Called once the value settles: the pointer is released, or a key press moves it. This is the value to save. */
	onChange: (value: number) => void;
}

function Slider({
	value,
	min,
	max,
	step,
	unit,
	onInput,
	onChange,
}: Omit<SliderRowProps, keyof SettingsRowProps>) {
	const { controlId, labelId, describedBy, disabled, invalid } = useSettingsRowControl();
	// What the thumb shows while it is held; the value in force the rest of the time.
	const [draft, setDraft] = useState<number | null>(null);
	const input = useRef<HTMLInputElement>(null);
	const settle = useRef<(value: number) => void>(onChange);
	settle.current = onChange;
	const shown = draft ?? value;

	// A value that arrives from outside (a reset, an import, another window) replaces whatever the thumb still shows, including a drag that ended without a native `change`.
	useEffect(() => {
		setDraft(null);
	}, [value]);

	// The native `change` event is the one that waits for the pointer to be released.
	useEffect(() => {
		const element = input.current;
		if (!element) return;
		const onSettled = (): void => {
			const next = Number(element.value);
			setDraft(null);
			if (Number.isFinite(next)) settle.current(next);
		};
		element.addEventListener('change', onSettled);
		return () => element.removeEventListener('change', onSettled);
	}, []);

	return (
		<span className={styles.sliderWrap}>
			<input
				ref={input}
				type="range"
				id={controlId}
				aria-labelledby={labelId}
				aria-describedby={describedBy}
				aria-invalid={invalid || undefined}
				aria-valuetext={unit ? `${shown}${unit}` : undefined}
				disabled={disabled}
				className={styles.slider}
				value={shown}
				min={min}
				max={max}
				step={step}
				onChange={(event) => {
					const next = Number(event.target.value);
					setDraft(next);
					onInput?.(next);
				}}
			/>
			<output htmlFor={controlId} className={styles.sliderValue}>
				{shown}
				{unit}
			</output>
		</span>
	);
}

export function SliderRow({
	value,
	min,
	max,
	step,
	unit,
	onInput,
	onChange,
	...row
}: SliderRowProps) {
	return (
		<SettingsRow {...row}>
			<Slider
				value={value}
				min={min}
				max={max}
				step={step}
				unit={unit}
				onInput={onInput}
				onChange={onChange}
			/>
		</SettingsRow>
	);
}
