// Tests for the settings shell: nav keyboard model, collapse, and the row components' semantics
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ButtonRow } from './ButtonRow';
import { LinkRow } from './LinkRow';
import { NumberRow } from './NumberRow';
import { SegmentedRow } from './SegmentedRow';
import { SelectRow } from './SelectRow';
import { SettingsGroup } from './SettingsGroup';
import { SettingsRow } from './SettingsRow';
import { SettingsSection } from './SettingsSection';
import { SettingsShell } from './SettingsShell';
import { SliderRow } from './SliderRow';
import { ToggleRow } from './ToggleRow';
import type { SettingsSectionDef } from './types';

type ResizeCallback = (entries: { contentRect: { width: number } }[]) => void;
let resize: ResizeCallback | null = null;
let width = 900;

beforeEach(() => {
	width = 900;
	resize = null;
	vi.stubGlobal(
		'ResizeObserver',
		class {
			constructor(callback: ResizeCallback) {
				resize = callback;
			}
			observe() {}
			disconnect() {}
		},
	);
	vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(
		() => ({ width, height: 600 }) as DOMRect,
	);
});

afterEach(() => {
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

const sections: SettingsSectionDef[] = [
	{
		id: 'general',
		label: 'General',
		icon: <svg data-testid="icon" />,
		render: () => <p>General page</p>,
	},
	{ id: 'operations', label: 'Operations', render: () => <p>Operations page</p> },
	{ id: 'dnd', label: 'Drag & drop', render: () => <p>Drag page</p> },
];

function Host({ initial = 'general' }: { initial?: string }) {
	const [id, setId] = useState(initial);
	return <SettingsShell sections={sections} activeId={id} onSelect={setId} />;
}

describe('SettingsShell navigation', () => {
	it('lists the sections in a labelled nav and marks the active one', () => {
		render(<Host />);
		const nav = screen.getByRole('navigation', { name: 'Settings sections' });
		expect(nav).toBeInTheDocument();
		expect(screen.getByRole('button', { name: 'General' })).toHaveAttribute('aria-current', 'page');
		expect(screen.getByRole('button', { name: 'Operations' })).not.toHaveAttribute('aria-current');
		expect(screen.getByTestId('icon')).toBeInTheDocument();
	});

	it('shows the active section as the heading and renders only its page', () => {
		render(<Host />);
		expect(screen.getByRole('heading', { name: 'General' })).toBeInTheDocument();
		expect(screen.getByRole('main')).toHaveAccessibleName('General');
		expect(screen.getByText('General page')).toBeInTheDocument();
		expect(screen.queryByText('Operations page')).toBeNull();
	});

	it('selects a section on click', async () => {
		render(<Host />);
		await userEvent.click(screen.getByRole('button', { name: 'Operations' }));
		expect(screen.getByText('Operations page')).toBeInTheDocument();
		expect(screen.getByRole('button', { name: 'Operations' })).toHaveAttribute(
			'aria-current',
			'page',
		);
	});

	it('uses a roving tabindex on the active item', () => {
		render(<Host initial="operations" />);
		expect(screen.getByRole('button', { name: 'Operations' })).toHaveAttribute('tabindex', '0');
		expect(screen.getByRole('button', { name: 'General' })).toHaveAttribute('tabindex', '-1');
	});

	it('moves focus with the arrow keys, wrapping, and with Home and End', async () => {
		const user = userEvent.setup();
		render(<Host />);
		screen.getByRole('button', { name: 'General' }).focus();
		await user.keyboard('{ArrowDown}');
		expect(screen.getByRole('button', { name: 'Operations' })).toHaveFocus();
		await user.keyboard('{End}');
		expect(screen.getByRole('button', { name: 'Drag & drop' })).toHaveFocus();
		await user.keyboard('{ArrowDown}');
		expect(screen.getByRole('button', { name: 'General' })).toHaveFocus();
		await user.keyboard('{ArrowUp}');
		expect(screen.getByRole('button', { name: 'Drag & drop' })).toHaveFocus();
		await user.keyboard('{Home}');
		expect(screen.getByRole('button', { name: 'General' })).toHaveFocus();
	});

	it('activates the focused item with Enter and Space, not on arrow alone', async () => {
		const user = userEvent.setup();
		render(<Host />);
		screen.getByRole('button', { name: 'General' }).focus();
		await user.keyboard('{ArrowDown}');
		expect(screen.getByText('General page')).toBeInTheDocument();
		await user.keyboard('{Enter}');
		expect(screen.getByText('Operations page')).toBeInTheDocument();
		await user.keyboard('{ArrowDown}');
		await user.keyboard(' ');
		expect(screen.getByText('Drag page')).toBeInTheDocument();
	});

	it('falls back to the first section for an unknown id', () => {
		render(<SettingsShell sections={sections} activeId="gone" onSelect={() => {}} />);
		expect(screen.getByRole('heading', { name: 'General' })).toBeInTheDocument();
	});

	it('renders a toolbar slot above the heading', () => {
		render(
			<SettingsShell
				sections={sections}
				activeId="general"
				onSelect={() => {}}
				toolbar={<div>Search slot</div>}
			/>,
		);
		expect(screen.getByText('Search slot')).toBeInTheDocument();
	});

	it('accepts translated labels', () => {
		render(
			<SettingsShell
				sections={sections}
				activeId="general"
				onSelect={() => {}}
				labels={{ navigation: 'Sections de réglages' }}
			/>,
		);
		expect(screen.getByRole('navigation', { name: 'Sections de réglages' })).toBeInTheDocument();
	});
});

describe('SettingsShell collapse', () => {
	it('keeps the side nav above the width token', () => {
		render(<Host />);
		expect(screen.getByRole('navigation')).toBeInTheDocument();
		expect(screen.queryByRole('combobox')).toBeNull();
	});

	it('collapses to a select under the width and back when it grows', async () => {
		render(<Host />);
		act(() => resize?.([{ contentRect: { width: 400 } }]));
		const select = screen.getByRole('combobox', { name: 'Settings sections' });
		expect(screen.queryByRole('navigation')).toBeNull();
		expect(select).toHaveValue('general');
		await userEvent.selectOptions(select, 'dnd');
		expect(screen.getByText('Drag page')).toBeInTheDocument();
		act(() => resize?.([{ contentRect: { width: 900 } }]));
		expect(screen.getByRole('navigation')).toBeInTheDocument();
	});

	it('starts collapsed when the first measurement is narrow', () => {
		width = 320;
		render(<Host />);
		expect(screen.getByRole('combobox', { name: 'Settings sections' })).toBeInTheDocument();
	});

	it('honours collapseBelow', () => {
		render(
			<SettingsShell
				sections={sections}
				activeId="general"
				onSelect={() => {}}
				collapseBelow={1000}
			/>,
		);
		expect(screen.getByRole('combobox')).toBeInTheDocument();
	});

	it('stays expanded without ResizeObserver', () => {
		vi.stubGlobal('ResizeObserver', undefined);
		render(<Host />);
		expect(screen.getByRole('navigation')).toBeInTheDocument();
	});
});

describe('SettingsGroup and SettingsSection', () => {
	it('names a group from its title', () => {
		render(
			<SettingsSection description="Intro text">
				<SettingsGroup title="Startup" description="When it starts">
					<p>row</p>
				</SettingsGroup>
			</SettingsSection>,
		);
		expect(screen.getByRole('group', { name: 'Startup' })).toBeInTheDocument();
		expect(screen.getByRole('heading', { level: 3, name: 'Startup' })).toBeInTheDocument();
		expect(screen.getByText('Intro text')).toBeInTheDocument();
		expect(screen.getByText('When it starts')).toBeInTheDocument();
	});
});

describe('SettingsRow', () => {
	it('labels and describes a custom control through the row wiring', () => {
		render(
			<SettingsRow label="Name" description="Shown in the title bar">
				<input />
			</SettingsRow>,
		);
		expect(screen.getByText('Name')).toBeInTheDocument();
		expect(screen.getByText('Shown in the title bar')).toBeInTheDocument();
	});

	it('shows a reason line and disables the control when unavailable', () => {
		render(
			<ToggleRow
				label="Use the desktop portal"
				description="Opens files through the portal"
				checked={false}
				onChange={() => {}}
				unavailableReason="no portal service was found"
			/>,
		);
		const toggle = screen.getByRole('switch', { name: 'Use the desktop portal' });
		expect(toggle).toBeDisabled();
		expect(screen.getByText('Unavailable: no portal service was found')).toBeInTheDocument();
		expect(toggle).toHaveAccessibleDescription(
			'Opens files through the portal Unavailable: no portal service was found',
		);
	});

	it('shows an error as an alert tied to the control, and clears it with the prop', () => {
		const { rerender } = render(
			<NumberRow
				label="Delay"
				description="How long to wait"
				value={600}
				onChange={() => {}}
				error="Delay must be between 200 and 2000"
			/>,
		);
		const field = screen.getByRole('spinbutton', { name: 'Delay' });
		expect(screen.getByRole('alert')).toHaveTextContent('Delay must be between 200 and 2000');
		expect(field).toHaveAttribute('aria-invalid', 'true');
		expect(field).toHaveAccessibleDescription(
			'How long to wait Delay must be between 200 and 2000',
		);
		rerender(<NumberRow label="Delay" value={600} onChange={() => {}} />);
		expect(screen.queryByRole('alert')).toBeNull();
		expect(screen.getByRole('spinbutton')).not.toHaveAttribute('aria-invalid');
	});

	it('marks a select invalid while its row shows an error', () => {
		render(
			<SelectRow
				label="Rule"
				value="a"
				options={[{ value: 'a', label: 'A' }]}
				onChange={() => {}}
				error="Refused"
			/>,
		);
		expect(screen.getByRole('combobox', { name: 'Rule' })).toHaveAttribute('aria-invalid', 'true');
	});

	it('disables the control without a reason line when only disabled', () => {
		render(<ToggleRow label="Thing" checked onChange={() => {}} disabled />);
		expect(screen.getByRole('switch')).toBeDisabled();
		expect(screen.queryByText(/Unavailable/)).toBeNull();
	});
});

describe('ToggleRow', () => {
	it('is a switch named by its label and reports its state', async () => {
		const onChange = vi.fn();
		const { rerender } = render(
			<ToggleRow
				label="Show hidden files"
				description="Dotfiles too"
				checked={false}
				onChange={onChange}
			/>,
		);
		const toggle = screen.getByRole('switch', { name: 'Show hidden files' });
		expect(toggle).toHaveAttribute('aria-checked', 'false');
		expect(toggle).toHaveAccessibleDescription('Dotfiles too');
		await userEvent.click(toggle);
		expect(onChange).toHaveBeenCalledWith(true);
		rerender(<ToggleRow label="Show hidden files" checked onChange={onChange} />);
		expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'true');
		await userEvent.click(screen.getByRole('switch'));
		expect(onChange).toHaveBeenLastCalledWith(false);
	});

	it('toggles from the keyboard and by clicking its label', async () => {
		const onChange = vi.fn();
		render(<ToggleRow label="Wrap" checked={false} onChange={onChange} />);
		screen.getByRole('switch').focus();
		await userEvent.keyboard(' ');
		await userEvent.keyboard('{Enter}');
		expect(onChange).toHaveBeenCalledTimes(2);
		await userEvent.click(screen.getByText('Wrap'));
		expect(onChange).toHaveBeenCalledTimes(3);
	});
});

describe('SelectRow', () => {
	it('associates the label and reports the chosen value', async () => {
		const onChange = vi.fn();
		render(
			<SelectRow
				label="Confirm deletes"
				value="trash"
				options={[
					{ value: 'trash', label: 'Move to Trash' },
					{ value: 'ask', label: 'Ask each time' },
				]}
				onChange={onChange}
			/>,
		);
		const select = screen.getByRole('combobox', { name: 'Confirm deletes' });
		expect(select).toHaveValue('trash');
		await userEvent.selectOptions(select, 'ask');
		expect(onChange).toHaveBeenCalledWith('ask');
	});
});

describe('NumberRow', () => {
	function Number({ onChange = vi.fn() }: { onChange?: (n: number) => void }) {
		const [value, setValue] = useState(4);
		return (
			<NumberRow
				label="Parallel jobs"
				value={value}
				min={1}
				max={16}
				step={1}
				unit="jobs"
				onChange={(n) => {
					setValue(n);
					onChange(n);
				}}
			/>
		);
	}

	it('labels the field, passes bounds and shows the unit', () => {
		render(<Number />);
		const field = screen.getByRole('spinbutton', { name: 'Parallel jobs' });
		expect(field).toHaveValue(4);
		expect(field).toHaveAttribute('min', '1');
		expect(field).toHaveAttribute('max', '16');
		expect(field).toHaveAttribute('step', '1');
		expect(screen.getByText('jobs')).toBeInTheDocument();
	});

	it('reports in-range edits as they happen', () => {
		const onChange = vi.fn();
		render(<Number onChange={onChange} />);
		fireEvent.change(screen.getByRole('spinbutton'), { target: { value: '8' } });
		expect(onChange).toHaveBeenCalledWith(8);
	});

	it('clamps an out-of-range value when editing finishes', () => {
		const onChange = vi.fn();
		render(<Number onChange={onChange} />);
		const field = screen.getByRole('spinbutton');
		fireEvent.change(field, { target: { value: '99' } });
		expect(onChange).not.toHaveBeenCalled();
		fireEvent.blur(field);
		expect(onChange).toHaveBeenCalledWith(16);
		expect(field).toHaveValue(16);
	});

	it('applies a typed value only on Enter or blur when it commits on commit', () => {
		const onChange = vi.fn();
		render(
			<NumberRow label="Depth" value={4} min={1} max={200} commitOn="commit" onChange={onChange} />,
		);
		const field = screen.getByRole('spinbutton');
		fireEvent.change(field, { target: { value: '5' } });
		fireEvent.change(field, { target: { value: '50' } });
		expect(onChange).not.toHaveBeenCalled();
		fireEvent.keyDown(field, { key: 'Enter' });
		expect(onChange).toHaveBeenCalledTimes(1);
		expect(onChange).toHaveBeenCalledWith(50);
	});

	it('reverts an empty field on blur', () => {
		const onChange = vi.fn();
		render(<Number onChange={onChange} />);
		const field = screen.getByRole('spinbutton');
		fireEvent.change(field, { target: { value: '' } });
		fireEvent.blur(field);
		expect(onChange).not.toHaveBeenCalled();
		expect(field).toHaveValue(4);
	});
});

describe('SegmentedRow', () => {
	function Segmented({ onChange = vi.fn() }: { onChange?: (v: string) => void }) {
		const [value, setValue] = useState('list');
		return (
			<SegmentedRow
				label="Default view"
				value={value}
				options={[
					{ value: 'list', label: 'List' },
					{ value: 'grid', label: 'Grid' },
					{ value: 'cols', label: 'Columns', disabled: true },
				]}
				onChange={(v) => {
					setValue(v);
					onChange(v);
				}}
			/>
		);
	}

	it('is a labelled radio group with one checked radio in the tab order', () => {
		render(<Segmented />);
		expect(screen.getByRole('radiogroup', { name: 'Default view' })).toBeInTheDocument();
		expect(screen.getByRole('radio', { name: 'List' })).toBeChecked();
		expect(screen.getByRole('radio', { name: 'List' })).toHaveAttribute('tabindex', '0');
		expect(screen.getByRole('radio', { name: 'Grid' })).toHaveAttribute('tabindex', '-1');
		expect(screen.getByRole('radio', { name: 'Columns' })).toBeDisabled();
	});

	it('selects on click and with arrows, skipping disabled options', async () => {
		const onChange = vi.fn();
		const user = userEvent.setup();
		render(<Segmented onChange={onChange} />);
		await user.click(screen.getByRole('radio', { name: 'Grid' }));
		expect(onChange).toHaveBeenLastCalledWith('grid');
		await user.keyboard('{ArrowRight}');
		expect(onChange).toHaveBeenLastCalledWith('list');
		expect(screen.getByRole('radio', { name: 'List' })).toHaveFocus();
		await user.keyboard('{End}');
		expect(screen.getByRole('radio', { name: 'Grid' })).toBeChecked();
		await user.keyboard('{Home}');
		expect(screen.getByRole('radio', { name: 'List' })).toBeChecked();
	});
});

describe('ButtonRow and LinkRow', () => {
	it('runs the action and names the button with its row context', async () => {
		const onAction = vi.fn();
		render(<ButtonRow label="Diagnostics" actionLabel="Export logs…" onAction={onAction} />);
		const button = screen.getByRole('button', { name: /Export logs…/ });
		expect(button).toHaveAccessibleDescription('Diagnostics');
		await userEvent.click(button);
		expect(onAction).toHaveBeenCalledOnce();
	});

	it('disables the action button when unavailable', () => {
		render(
			<ButtonRow label="Sync" actionLabel="Run" onAction={() => {}} unavailableReason="offline" />,
		);
		expect(screen.getByRole('button')).toBeDisabled();
		expect(screen.getByText('Unavailable: offline')).toBeInTheDocument();
	});

	it('renders a link with its description', () => {
		render(
			<LinkRow label="Licences" description="Open source notices" href="https://example.com/l" />,
		);
		const link = screen.getByRole('link', { name: /Licences/ });
		expect(link).toHaveAttribute('href', 'https://example.com/l');
		expect(link).toHaveAccessibleName('Licences Open source notices');
	});

	it('hands activation to the app and cancels the navigation', () => {
		const onActivate = vi.fn();
		render(<LinkRow label="Docs" href="https://example.com" onActivate={onActivate} />);
		const notPrevented = fireEvent.click(screen.getByRole('link'));
		expect(onActivate).toHaveBeenCalledOnce();
		expect(notPrevented).toBe(false);
	});

	it('is not a link when unavailable', () => {
		render(<LinkRow label="Docs" href="https://example.com" unavailableReason="offline" />);
		expect(screen.queryByRole('link')).toBeNull();
		expect(screen.getByText('Unavailable: offline')).toBeInTheDocument();
	});
});

describe('SliderRow', () => {
	it('previews every step while it is dragged and applies the value that settles', () => {
		const onInput = vi.fn();
		const onChange = vi.fn();
		render(
			<SliderRow
				label="Opacity"
				value={80}
				min={40}
				max={100}
				unit="%"
				onInput={onInput}
				onChange={onChange}
			/>,
		);
		const slider = screen.getByRole('slider', { name: 'Opacity' });
		expect(slider).toHaveValue('80');
		// A drag is a run of `input` events; the page's preview follows, and nothing is saved yet.
		fireEvent.input(slider, { target: { value: '70' } });
		fireEvent.input(slider, { target: { value: '60' } });
		expect(onInput.mock.calls.map(([value]) => value)).toEqual([70, 60]);
		expect(onChange).not.toHaveBeenCalled();
		expect(screen.getByText('60%')).toBeInTheDocument();
		// Letting go is the native `change` event.
		fireEvent.change(slider);
		expect(onChange).toHaveBeenCalledExactlyOnceWith(60);
	});

	it('goes back to the value in force when the change is refused', () => {
		const onChange = vi.fn();
		render(<SliderRow label="Opacity" value={80} min={40} max={100} onChange={onChange} />);
		const slider = screen.getByRole('slider', { name: 'Opacity' });
		fireEvent.input(slider, { target: { value: '50' } });
		fireEvent.change(slider);
		expect(onChange).toHaveBeenCalledWith(50);
		// The parent keeps `value` at 80, as it does when Rust refuses the change.
		expect(slider).toHaveValue('80');
	});

	it('shows a value that arrives from outside, even after a drag that never settled', () => {
		const props = { label: 'Opacity', min: 40, max: 100, onChange: vi.fn() };
		const { rerender } = render(<SliderRow {...props} value={60} />);
		const slider = screen.getByRole('slider', { name: 'Opacity' });
		// A drag that ends where it began fires no native `change`, so the draft is never cleared.
		fireEvent.input(slider, { target: { value: '50' } });
		expect(slider).toHaveValue('50');
		// A reset (or an import, or another window) puts the value in force somewhere else.
		rerender(<SliderRow {...props} value={82} />);
		expect(slider).toHaveValue('82');
	});

	it('is dimmed and cannot be moved when its row is disabled', () => {
		render(<SliderRow label="Opacity" value={80} min={40} max={100} disabled onChange={vi.fn()} />);
		expect(screen.getByRole('slider', { name: 'Opacity' })).toBeDisabled();
	});
});
