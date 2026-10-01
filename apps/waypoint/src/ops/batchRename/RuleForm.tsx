// The small form for one rule of a batch rename: the fields of whichever type of rule it is
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { CaseMode } from '@liminal-hq/waypoint-protocol/generated/CaseMode';
import type { DateSource } from '@liminal-hq/waypoint-protocol/generated/DateSource';
import type { RenameRule } from '@liminal-hq/waypoint-protocol/generated/RenameRule';
import type { RenameScope } from '@liminal-hq/waypoint-protocol/generated/RenameScope';
import type { RulePosition } from '@liminal-hq/waypoint-protocol/generated/RulePosition';
import { useId, type ReactNode } from 'react';
import { t, type MessageId } from '../../i18n/messages';
import styles from './RuleForm.module.css';

export interface RuleFormProps {
	rule: RenameRule;
	onChange: (rule: RenameRule) => void;
}

/** A whole number from a number field, kept in `0..=max`; anything else is 0. */
function wholeNumber(text: string, max: number): number {
	const value = Math.floor(Number(text));
	return Number.isFinite(value) ? Math.min(Math.max(value, 0), max) : 0;
}

interface FieldProps {
	label: string;
	hint?: string;
	children: (ids: { id: string; hintId: string | undefined }) => ReactNode;
}

function Field({ label, hint, children }: FieldProps) {
	const id = useId();
	const hintId = hint ? `${id}-hint` : undefined;
	return (
		<div className={styles.field}>
			<label htmlFor={id} className={styles.label}>
				{label}
			</label>
			{children({ id, hintId })}
			{hint ? (
				<span id={hintId} className={styles.hint}>
					{hint}
				</span>
			) : null}
		</div>
	);
}

function TextField(props: {
	label: string;
	hint?: string;
	value: string;
	onChange: (value: string) => void;
	mono?: boolean;
}) {
	return (
		<Field label={props.label} hint={props.hint}>
			{({ id, hintId }) => (
				<input
					id={id}
					type="text"
					className={[styles.input, props.mono ? styles.mono : ''].filter(Boolean).join(' ')}
					value={props.value}
					aria-describedby={hintId}
					spellCheck={false}
					autoComplete="off"
					onChange={(event) => props.onChange(event.target.value)}
				/>
			)}
		</Field>
	);
}

function NumberField(props: {
	label: string;
	hint?: string;
	value: number;
	min?: number;
	max: number;
	onChange: (value: number) => void;
}) {
	return (
		<Field label={props.label} hint={props.hint}>
			{({ id, hintId }) => (
				<input
					id={id}
					type="number"
					className={[styles.input, styles.number].join(' ')}
					value={props.value}
					min={props.min ?? 0}
					max={props.max}
					step={1}
					aria-describedby={hintId}
					onChange={(event) => props.onChange(wholeNumber(event.target.value, props.max))}
				/>
			)}
		</Field>
	);
}

function SelectField<V extends string>(props: {
	label: string;
	value: V;
	options: ReadonlyArray<{ value: V; label: string }>;
	onChange: (value: V) => void;
}) {
	return (
		<Field label={props.label}>
			{({ id }) => (
				<select
					id={id}
					className={styles.input}
					value={props.value}
					onChange={(event) => props.onChange(event.target.value as V)}
				>
					{props.options.map((option) => (
						<option key={option.value} value={option.value}>
							{option.label}
						</option>
					))}
				</select>
			)}
		</Field>
	);
}

function CheckField(props: {
	label: string;
	hint?: string;
	checked: boolean;
	onChange: (checked: boolean) => void;
}) {
	const id = useId();
	const hintId = props.hint ? `${id}-hint` : undefined;
	return (
		<div className={styles.check}>
			<input
				id={id}
				type="checkbox"
				checked={props.checked}
				aria-describedby={hintId}
				onChange={(event) => props.onChange(event.target.checked)}
			/>
			<label htmlFor={id}>{props.label}</label>
			{props.hint ? (
				<span id={hintId} className={styles.hint}>
					{props.hint}
				</span>
			) : null}
		</div>
	);
}

function options<V extends string>(
	values: readonly V[],
	message: (value: V) => MessageId,
): Array<{ value: V; label: string }> {
	return values.map((value) => ({ value, label: t(message(value)) }));
}

const SCOPES = options<RenameScope>(
	['stem', 'name', 'extension'],
	(scope) => `batchRename.scope.${scope}`,
);
const POSITIONS = options<RulePosition>(
	['prefix', 'suffix', 'replaceStem'],
	(position) => `batchRename.position.${position}`,
);
const CASES = options<CaseMode>(
	['upper', 'lower', 'title', 'sentence'],
	(mode) => `batchRename.case.${mode}`,
);
const DATES = options<DateSource>(
	['modified', 'created', 'today'],
	(source) => `batchRename.date.${source}`,
);

type InsertWhere = 'start' | 'end' | 'index';
const INSERT_WHERE = options<InsertWhere>(
	['start', 'end', 'index'],
	(where) => `batchRename.insert.${where}`,
);

const U32_MAX = 4_294_967_295;

/** The fields of one rule. The rule is a value: every change calls `onChange` with a new one. */
export function RuleForm({ rule, onChange }: RuleFormProps) {
	switch (rule.kind) {
		case 'findReplace':
			return (
				<div className={styles.form}>
					<TextField
						label={t('batchRename.find')}
						value={rule.find}
						mono={rule.regex}
						onChange={(find) => onChange({ ...rule, find })}
					/>
					<TextField
						label={t('batchRename.replace')}
						value={rule.replace}
						mono={rule.regex}
						hint={rule.regex ? t('batchRename.regex.hint') : undefined}
						onChange={(replace) => onChange({ ...rule, replace })}
					/>
					<SelectField
						label={t('batchRename.scope.label')}
						value={rule.scope}
						options={SCOPES}
						onChange={(scope) => onChange({ ...rule, scope })}
					/>
					<div className={styles.checks}>
						<CheckField
							label={t('batchRename.regex')}
							checked={rule.regex}
							onChange={(regex) => onChange({ ...rule, regex })}
						/>
						<CheckField
							label={t('batchRename.caseSensitive')}
							checked={rule.caseSensitive}
							onChange={(caseSensitive) => onChange({ ...rule, caseSensitive })}
						/>
						<CheckField
							label={t('batchRename.all')}
							checked={rule.all}
							onChange={(all) => onChange({ ...rule, all })}
						/>
					</div>
				</div>
			);
		case 'counter':
			return (
				<div className={styles.form}>
					<NumberField
						label={t('batchRename.counter.start')}
						value={rule.start}
						max={U32_MAX}
						onChange={(start) => onChange({ ...rule, start })}
					/>
					<NumberField
						label={t('batchRename.counter.step')}
						value={rule.step}
						max={U32_MAX}
						onChange={(step) => onChange({ ...rule, step })}
					/>
					<NumberField
						label={t('batchRename.counter.width')}
						value={rule.width}
						max={32}
						onChange={(width) => onChange({ ...rule, width })}
					/>
					<SelectField
						label={t('batchRename.position.label')}
						value={rule.position}
						options={POSITIONS}
						onChange={(position) => onChange({ ...rule, position })}
					/>
					<TextField
						label={t('batchRename.separator')}
						value={rule.separator}
						onChange={(separator) => onChange({ ...rule, separator })}
					/>
				</div>
			);
		case 'case':
			return (
				<div className={styles.form}>
					<SelectField
						label={t('batchRename.case.mode')}
						value={rule.mode}
						options={CASES}
						onChange={(mode) => onChange({ ...rule, mode })}
					/>
					<SelectField
						label={t('batchRename.scope.label')}
						value={rule.scope}
						options={SCOPES}
						onChange={(scope) => onChange({ ...rule, scope })}
					/>
				</div>
			);
		case 'dateToken':
			return (
				<div className={styles.form}>
					<SelectField
						label={t('batchRename.date.source')}
						value={rule.source}
						options={DATES}
						onChange={(source) => onChange({ ...rule, source })}
					/>
					<TextField
						label={t('batchRename.date.format')}
						hint={t('batchRename.date.hint')}
						value={rule.format}
						mono
						onChange={(format) => onChange({ ...rule, format })}
					/>
					<SelectField
						label={t('batchRename.position.label')}
						value={rule.position}
						options={POSITIONS}
						onChange={(position) => onChange({ ...rule, position })}
					/>
					<TextField
						label={t('batchRename.separator')}
						value={rule.separator}
						onChange={(separator) => onChange({ ...rule, separator })}
					/>
				</div>
			);
		case 'insert': {
			const where: InsertWhere = rule.at.kind;
			return (
				<div className={styles.form}>
					<TextField
						label={t('batchRename.insert.text')}
						value={rule.text}
						onChange={(text) => onChange({ ...rule, text })}
					/>
					<SelectField
						label={t('batchRename.insert.at')}
						value={where}
						options={INSERT_WHERE}
						onChange={(next) =>
							onChange({
								...rule,
								at: next === 'index' ? { kind: 'index', index: 0 } : { kind: next },
							})
						}
					/>
					{rule.at.kind === 'index' ? (
						<NumberField
							label={t('batchRename.insert.position')}
							value={rule.at.index + 1}
							min={1}
							max={U32_MAX}
							onChange={(position) =>
								onChange({ ...rule, at: { kind: 'index', index: Math.max(position, 1) - 1 } })
							}
						/>
					) : null}
				</div>
			);
		}
		case 'remove':
			return (
				<div className={styles.form}>
					<NumberField
						label={t('batchRename.remove.from')}
						hint={t('batchRename.remove.hint')}
						value={rule.from + 1}
						min={1}
						max={U32_MAX}
						onChange={(from) => onChange({ ...rule, from: Math.max(from, 1) - 1 })}
					/>
					<NumberField
						label={t('batchRename.remove.to')}
						value={rule.to}
						max={U32_MAX}
						onChange={(to) => onChange({ ...rule, to })}
					/>
				</div>
			);
		case 'trimWhitespace':
			return <p className={styles.note}>{t('batchRename.trim.note')}</p>;
		case 'changeExtension':
			return (
				<div className={styles.form}>
					<TextField
						label={t('batchRename.extension.to')}
						hint={t('batchRename.extension.hint')}
						value={rule.to}
						onChange={(to) => onChange({ ...rule, to })}
					/>
				</div>
			);
	}
}
