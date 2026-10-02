// Settings row: label, description and a control slot with disabled and unavailable states
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useContext, useId, type ReactNode } from 'react';
import '../tokens.css';
import { useSettingsLabels } from './labels';
import styles from './SettingsRow.module.css';

export interface SettingsRowProps {
	label: ReactNode;
	description?: ReactNode;
	/** The control. Typed rows fill this in; a custom control reads `useSettingsRowControl()`. */
	children?: ReactNode;
	disabled?: boolean;
	/**
	 * Marks an option this system cannot offer and says why. The row stays visible and dimmed with a
	 * reason line, for the Services-panel convention. An app that hides such options just omits the row.
	 */
	unavailableReason?: string;
	/**
	 * Why the last change was refused, shown under the description as an alert and tied to the
	 * control. The app clears it when the next change is made.
	 */
	error?: string;
	/** Set false when the control's own text is its name (a button), so the label is not tied to it. Default true. */
	associateLabel?: boolean;
}

/** What a control inside a row needs to be labelled, described and disabled. */
export interface SettingsRowControl {
	/** The id to put on the control. */
	controlId: string;
	labelId: string;
	/** Space-separated ids of the description and reason lines that exist. */
	describedBy: string | undefined;
	disabled: boolean;
	/** The row shows an error, so a field marks itself invalid. */
	invalid: boolean;
}

const RowContext = createContext<SettingsRowControl | null>(null);

/** Wiring for the control inside the nearest `SettingsRow`. */
export function useSettingsRowControl(): SettingsRowControl {
	const control = useContext(RowContext);
	if (!control) throw new Error('useSettingsRowControl must be used inside a SettingsRow');
	return control;
}

export function SettingsRow({
	label,
	description,
	children,
	disabled = false,
	unavailableReason,
	error,
	associateLabel = true,
}: SettingsRowProps) {
	const id = useId();
	const labels = useSettingsLabels();
	const unavailable = unavailableReason !== undefined;
	const off = disabled || unavailable;
	const descriptionId = description ? `${id}-description` : undefined;
	const reasonId = unavailable ? `${id}-reason` : undefined;
	const errorId = error ? `${id}-error` : undefined;
	const describedBy = [descriptionId, reasonId, errorId].filter(Boolean).join(' ') || undefined;
	const control: SettingsRowControl = {
		controlId: `${id}-control`,
		labelId: `${id}-label`,
		describedBy,
		disabled: off,
		invalid: error !== undefined,
	};
	return (
		<RowContext.Provider value={control}>
			<div
				className={[styles.row, off ? styles.off : '', unavailable ? styles.unavailable : '']
					.filter(Boolean)
					.join(' ')}
				data-unavailable={unavailable ? '' : undefined}
			>
				<div className={styles.text}>
					<label
						id={control.labelId}
						htmlFor={associateLabel ? control.controlId : undefined}
						className={styles.label}
					>
						{label}
					</label>
					{description ? (
						<div id={descriptionId} className={styles.description}>
							{description}
						</div>
					) : null}
					{unavailable ? (
						<div id={reasonId} className={styles.reason}>
							{labels.unavailable}: {unavailableReason}
						</div>
					) : null}
					{error ? (
						<div id={errorId} role="alert" className={styles.error}>
							{error}
						</div>
					) : null}
				</div>
				<div className={styles.control}>{children}</div>
			</div>
		</RowContext.Provider>
	);
}
