// Verifies the action picker: its items, its keyboard, and that dismissing it is a cancel
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { expectEveryItemHasIcon } from '@liminal-hq/waypoint-chrome/ContextMenu/expectEveryItemHasIcon';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ActionPicker, pickerItems } from './ActionPicker';
import type { PickerRequest } from './fileDrag';

afterEach(cleanup);

const request = (verbs: PickerRequest['verbs'] = ['copy', 'move', 'link']): PickerRequest => ({
	position: { x: 40, y: 50 },
	what: 'a.txt',
	target: {
		kind: 'place',
		ref: 'x',
		label: 'Docs',
		location: { display: '/docs', uri: 'file:///docs' },
		outcome: 'ask',
		blocked: null,
		volume: 'same',
		pending: false,
	},
	verbs,
	choose: vi.fn(),
	cancel: vi.fn(),
});

describe('pickerItems', () => {
	it('lists Copy Here, Move Here, Link Here and Cancel, each with an icon', () => {
		const items = pickerItems(['copy', 'move', 'link']);
		expect(items.flatMap((item) => (item.type === 'separator' ? [] : [item.id]))).toEqual([
			'copy',
			'move',
			'link',
			'cancel',
		]);
		expectEveryItemHasIcon(items);
	});

	it('offers Compress Here and Extract Here, and reads Copy as Add to Archive in an archive', () => {
		const items = pickerItems(['copy', 'compress', 'extract'], true);
		expect(items.flatMap((item) => (item.type === 'separator' ? [] : [item.id]))).toEqual([
			'copy',
			'compress',
			'extract',
			'cancel',
		]);
		expect(items.flatMap((item) => (item.type === 'action' ? [item.label] : []))).toEqual([
			'Add to Archive',
			'Compress Here…',
			'Extract Here',
			'Cancel',
		]);
		expectEveryItemHasIcon(items);
	});

	it('leaves out what is not on offer', () => {
		expect(pickerItems(['copy']).filter((item) => item.type === 'action')).toHaveLength(2);
	});
});

describe('ActionPicker', () => {
	it('is a menu named Drop action, with the verbs as its items', () => {
		render(<ActionPicker request={request()} onDone={vi.fn()} />);
		const menu = screen.getByRole('menu', { name: 'Drop action' });
		expect(menu).toBeInTheDocument();
		expect(screen.getAllByRole('menuitem').map((item) => item.textContent)).toEqual([
			'Copy Here',
			'Move Here',
			'Link Here',
			'Cancel',
		]);
	});

	it('chooses with the pointer', () => {
		const picker = request();
		const onDone = vi.fn();
		render(<ActionPicker request={picker} onDone={onDone} />);
		fireEvent.click(screen.getByRole('menuitem', { name: 'Move Here' }));
		expect(picker.choose).toHaveBeenCalledWith('move');
		expect(picker.cancel).not.toHaveBeenCalled();
		expect(onDone).toHaveBeenCalled();
	});

	it('chooses with the keyboard: arrows move, Enter chooses', async () => {
		const picker = request();
		const user = userEvent.setup();
		render(<ActionPicker request={picker} onDone={vi.fn()} />);
		await user.keyboard('{ArrowDown}{ArrowDown}{Enter}');
		expect(picker.choose).toHaveBeenCalledWith('move');
	});

	it('cancels with Escape, with Cancel and by clicking away, once each', async () => {
		const escape = request();
		const onDone = vi.fn();
		const { unmount } = render(<ActionPicker request={escape} onDone={onDone} />);
		await userEvent.setup().keyboard('{Escape}');
		expect(escape.cancel).toHaveBeenCalledTimes(1);
		expect(escape.choose).not.toHaveBeenCalled();
		unmount();

		const button = request();
		render(<ActionPicker request={button} onDone={vi.fn()} />);
		fireEvent.click(screen.getByRole('menuitem', { name: 'Cancel' }));
		expect(button.cancel).toHaveBeenCalledTimes(1);
		expect(button.choose).not.toHaveBeenCalled();
		cleanup();

		const away = request();
		render(<ActionPicker request={away} onDone={vi.fn()} />);
		fireEvent.pointerDown(document.body);
		expect(away.cancel).toHaveBeenCalledTimes(1);
	});
});
