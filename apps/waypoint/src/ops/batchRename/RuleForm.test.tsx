// Tests for the per-rule forms: each type shows its fields and reports an edit as a new rule
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { RenameRule } from '@liminal-hq/waypoint-protocol/generated/RenameRule';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { defaultRule, RULE_KINDS } from './batchRenameModel';
import { RuleForm } from './RuleForm';

afterEach(cleanup);

function show(rule: RenameRule) {
	const onChange = vi.fn();
	render(<RuleForm rule={rule} onChange={onChange} />);
	return onChange;
}

describe('RuleForm', () => {
	it('labels every control of every rule type', () => {
		for (const kind of RULE_KINDS) {
			const { container, unmount } = render(
				<RuleForm rule={defaultRule(kind)} onChange={() => {}} />,
			);
			for (const control of container.querySelectorAll('input, select')) {
				expect(
					(control as HTMLInputElement).labels?.length ?? 0,
					`${kind}: a control has no label`,
				).toBeGreaterThan(0);
			}
			unmount();
		}
	});

	it('edits a find and replace as typed', async () => {
		const rule = defaultRule('findReplace');
		const onChange = show(rule);
		await userEvent.type(screen.getByLabelText('Find'), 'a');
		expect(onChange).toHaveBeenLastCalledWith({ ...rule, find: 'a' });
		await userEvent.type(screen.getByLabelText('Replace with'), 'b');
		expect(onChange).toHaveBeenLastCalledWith({ ...rule, replace: 'b' });
		await userEvent.click(screen.getByLabelText('Regular expression'));
		expect(onChange).toHaveBeenLastCalledWith({ ...rule, regex: true });
		await userEvent.click(screen.getByLabelText('Match case'));
		expect(onChange).toHaveBeenLastCalledWith({ ...rule, caseSensitive: true });
		await userEvent.click(screen.getByLabelText('Replace every match'));
		expect(onChange).toHaveBeenLastCalledWith({ ...rule, all: false });
		await userEvent.selectOptions(screen.getByLabelText('Apply to'), 'extension');
		expect(onChange).toHaveBeenLastCalledWith({ ...rule, scope: 'extension' });
	});

	it('hints at groups only for a regular expression', () => {
		const rule = defaultRule('findReplace');
		if (rule.kind !== 'findReplace') throw new Error('unreachable');
		const { rerender } = render(<RuleForm rule={rule} onChange={() => {}} />);
		expect(screen.queryByText(/for a group/)).toBeNull();
		rerender(<RuleForm rule={{ ...rule, regex: true }} onChange={() => {}} />);
		expect(screen.getByText(/for a group/)).toBeInTheDocument();
		expect(screen.getByLabelText('Replace with')).toHaveAccessibleDescription(/for a group/);
	});

	it('edits a counter and keeps its numbers whole and in range', () => {
		const rule = defaultRule('counter');
		const onChange = show(rule);
		fireEvent.change(screen.getByLabelText('Start at'), { target: { value: '5' } });
		expect(onChange).toHaveBeenLastCalledWith({ ...rule, start: 5 });
		fireEvent.change(screen.getByLabelText('Step'), { target: { value: '-3' } });
		expect(onChange).toHaveBeenLastCalledWith({ ...rule, step: 0 });
		fireEvent.change(screen.getByLabelText('Digits'), { target: { value: '99' } });
		expect(onChange).toHaveBeenLastCalledWith({ ...rule, width: 32 });
		fireEvent.change(screen.getByLabelText('Digits'), { target: { value: '2.9' } });
		expect(onChange).toHaveBeenLastCalledWith({ ...rule, width: 2 });
		fireEvent.change(screen.getByLabelText('Start at'), { target: { value: '' } });
		expect(onChange).toHaveBeenLastCalledWith({ ...rule, start: 0 });
	});

	it('moves a counter before, after or in place of the name', async () => {
		const rule = defaultRule('counter');
		const onChange = show(rule);
		await userEvent.selectOptions(screen.getByLabelText('Position'), 'replaceStem');
		expect(onChange).toHaveBeenLastCalledWith({ ...rule, position: 'replaceStem' });
		await userEvent.type(screen.getByLabelText('Separator'), '_');
		expect(onChange).toHaveBeenLastCalledWith({
			...rule,
			separator: `${'separator' in rule ? rule.separator : ''}_`,
		});
	});

	it('picks a case and where it applies', async () => {
		const rule = defaultRule('case');
		const onChange = show(rule);
		await userEvent.selectOptions(screen.getByLabelText('Change to'), 'upper');
		expect(onChange).toHaveBeenLastCalledWith({ ...rule, mode: 'upper' });
		await userEvent.selectOptions(screen.getByLabelText('Apply to'), 'name');
		expect(onChange).toHaveBeenLastCalledWith({ ...rule, scope: 'name' });
	});

	it('edits a date from a time, a format and a place', async () => {
		const rule = defaultRule('dateToken');
		const onChange = show(rule);
		await userEvent.selectOptions(screen.getByLabelText('Date from'), 'today');
		expect(onChange).toHaveBeenLastCalledWith({ ...rule, source: 'today' });
		fireEvent.change(screen.getByLabelText('Format'), { target: { value: '%Y%m' } });
		expect(onChange).toHaveBeenLastCalledWith({ ...rule, format: '%Y%m' });
		expect(screen.getByLabelText('Format')).toHaveAccessibleDescription(/Year %Y/);
		await userEvent.selectOptions(screen.getByLabelText('Position'), 'suffix');
		expect(onChange).toHaveBeenLastCalledWith({ ...rule, position: 'suffix' });
	});

	it('inserts text at the start, the end or a character, counting from 1', async () => {
		const rule = defaultRule('insert');
		const onChange = show(rule);
		await userEvent.type(screen.getByLabelText('Text'), 'x');
		expect(onChange).toHaveBeenLastCalledWith({ ...rule, text: 'x' });
		await userEvent.selectOptions(screen.getByLabelText('Insert at'), 'index');
		expect(onChange).toHaveBeenLastCalledWith({ ...rule, at: { kind: 'index', index: 0 } });
		await userEvent.selectOptions(screen.getByLabelText('Insert at'), 'start');
		expect(onChange).toHaveBeenLastCalledWith({ ...rule, at: { kind: 'start' } });
	});

	it('shows the character position of an insert only when it is at one', () => {
		const rule = defaultRule('insert');
		const { rerender } = render(<RuleForm rule={rule} onChange={() => {}} />);
		expect(screen.queryByLabelText('Character position')).toBeNull();
		const at = { ...rule, at: { kind: 'index', index: 2 } } as RenameRule;
		const onChange = vi.fn();
		rerender(<RuleForm rule={at} onChange={onChange} />);
		const field = screen.getByLabelText('Character position');
		expect(field).toHaveValue(3);
		fireEvent.change(field, { target: { value: '1' } });
		expect(onChange).toHaveBeenLastCalledWith({ ...at, at: { kind: 'index', index: 0 } });
		fireEvent.change(field, { target: { value: '0' } });
		expect(onChange).toHaveBeenLastCalledWith({ ...at, at: { kind: 'index', index: 0 } });
	});

	it('removes characters counted from 1, both ends included', () => {
		const rule = defaultRule('remove');
		const onChange = show(rule);
		expect(screen.getByLabelText('From character')).toHaveValue(1);
		expect(screen.getByLabelText('To character')).toHaveValue(1);
		fireEvent.change(screen.getByLabelText('From character'), { target: { value: '3' } });
		expect(onChange).toHaveBeenLastCalledWith({ ...rule, from: 2 });
		fireEvent.change(screen.getByLabelText('To character'), { target: { value: '5' } });
		expect(onChange).toHaveBeenLastCalledWith({ ...rule, to: 5 });
	});

	it('trims with nothing to fill in, and changes an extension with one field', async () => {
		const { unmount } = render(
			<RuleForm rule={defaultRule('trimWhitespace')} onChange={() => {}} />,
		);
		expect(screen.getByText(/Removes spaces from both ends/)).toBeInTheDocument();
		expect(document.querySelectorAll('input, select')).toHaveLength(0);
		unmount();
		const rule = defaultRule('changeExtension');
		const onChange = show(rule);
		await userEvent.type(screen.getByLabelText('New extension'), 'm');
		expect(onChange).toHaveBeenLastCalledWith({ ...rule, to: 'm' });
		expect(screen.getByLabelText('New extension')).toHaveAccessibleDescription(
			/Folders are left alone/,
		);
	});
});
