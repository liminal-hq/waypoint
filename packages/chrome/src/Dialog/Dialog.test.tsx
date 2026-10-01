// Tests for the dialog: native open and close, dismissal rules, focus, stacking and the Tab trap
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { createRef, useState } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ConfirmDialog } from './ConfirmDialog';
import { Dialog, type DialogProps } from './Dialog';
import { DialogActions, DialogButton } from './DialogActions';

function renderDialog(props: Partial<DialogProps> = {}) {
	const onClose = vi.fn();
	const utils = render(
		<Dialog open title="Rename" onClose={onClose} {...props}>
			<input aria-label="Name" />
			<input aria-label="Extension" />
		</Dialog>,
	);
	return { onClose, ...utils };
}

afterEach(() => {
	document.documentElement.removeAttribute('style');
});

describe('Dialog open and close', () => {
	it('renders nothing while closed', () => {
		renderDialog({ open: false });
		expect(document.querySelector('dialog')).toBeNull();
	});

	it('opens the native dialog as a modal', () => {
		const show = vi.spyOn(HTMLDialogElement.prototype, 'showModal');
		renderDialog();
		expect(document.querySelector('dialog')).toHaveAttribute('open');
		expect(show).toHaveBeenCalledOnce();
		show.mockRestore();
	});

	it('closes when the app sets open to false', () => {
		const { rerender } = renderDialog();
		rerender(<Dialog open={false} title="Rename" onClose={() => {}} />);
		expect(document.querySelector('dialog')).toBeNull();
	});

	it('falls back to the open attribute without showModal', () => {
		const original = HTMLDialogElement.prototype.showModal;
		// @ts-expect-error simulate a webview without the method
		HTMLDialogElement.prototype.showModal = undefined;
		try {
			renderDialog();
			const dialog = document.querySelector('dialog');
			expect(dialog).toHaveAttribute('open');
			expect(dialog?.className).toMatch(/fallback/);
		} finally {
			HTMLDialogElement.prototype.showModal = original;
		}
	});

	it('locks page scrolling while open and releases it on close', () => {
		const { rerender } = renderDialog();
		expect(document.documentElement.style.overflow).toBe('hidden');
		rerender(<Dialog open={false} title="Rename" onClose={() => {}} />);
		expect(document.documentElement.style.overflow).toBe('');
	});

	it('keeps the lock until the last stacked dialog closes', () => {
		const { rerender } = render(
			<>
				<Dialog open title="One" onClose={() => {}} />
				<Dialog open title="Two" onClose={() => {}} />
			</>,
		);
		rerender(
			<>
				<Dialog open title="One" onClose={() => {}} />
				<Dialog open={false} title="Two" onClose={() => {}} />
			</>,
		);
		expect(document.documentElement.style.overflow).toBe('hidden');
		rerender(
			<>
				<Dialog open={false} title="One" onClose={() => {}} />
				<Dialog open={false} title="Two" onClose={() => {}} />
			</>,
		);
		expect(document.documentElement.style.overflow).toBe('');
	});
});

describe('Dialog naming', () => {
	it('names the dialog from its title and describes it', () => {
		renderDialog({ title: 'Delete files', description: 'This cannot be undone.' });
		const dialog = screen.getByRole('dialog', { name: 'Delete files' });
		expect(dialog).toHaveAccessibleDescription('This cannot be undone.');
	});

	it('has no description reference without a description', () => {
		renderDialog();
		expect(document.querySelector('dialog')).not.toHaveAttribute('aria-describedby');
	});
});

describe('Dialog dismissal', () => {
	it('asks to close with "escape" on Esc', async () => {
		const { onClose } = renderDialog();
		await userEvent.keyboard('{Escape}');
		expect(onClose).toHaveBeenCalledExactlyOnceWith('escape');
	});

	it('answers a native cancel event once, as "cancel", and keeps the dialog open', () => {
		const { onClose } = renderDialog();
		const dialog = document.querySelector('dialog') as HTMLDialogElement;
		const event = new Event('cancel', { cancelable: true });
		dialog.dispatchEvent(event);
		expect(event.defaultPrevented).toBe(true);
		expect(onClose).toHaveBeenCalledExactlyOnceWith('cancel');
		expect(dialog).toHaveAttribute('open');
	});

	it('asks to close with "backdrop" when press and release land on the backdrop', () => {
		const { onClose } = renderDialog();
		const dialog = document.querySelector('dialog') as HTMLDialogElement;
		fireEvent.mouseDown(dialog);
		fireEvent.click(dialog);
		expect(onClose).toHaveBeenCalledExactlyOnceWith('backdrop');
	});

	it('ignores a click that began inside the dialog and ended on the backdrop', () => {
		const { onClose } = renderDialog();
		const dialog = document.querySelector('dialog') as HTMLDialogElement;
		fireEvent.mouseDown(screen.getByLabelText('Name'));
		fireEvent.click(dialog);
		expect(onClose).not.toHaveBeenCalled();
	});

	it('ignores clicks inside the dialog', () => {
		const { onClose } = renderDialog();
		fireEvent.mouseDown(screen.getByLabelText('Name'));
		fireEvent.click(screen.getByLabelText('Name'));
		expect(onClose).not.toHaveBeenCalled();
	});

	it('does not dismiss on Esc, cancel or backdrop when not dismissible', async () => {
		const { onClose } = renderDialog({ dismissible: false });
		const dialog = document.querySelector('dialog') as HTMLDialogElement;
		await userEvent.keyboard('{Escape}');
		dialog.dispatchEvent(new Event('cancel', { cancelable: true }));
		fireEvent.mouseDown(dialog);
		fireEvent.click(dialog);
		expect(onClose).not.toHaveBeenCalled();
		expect(dialog).toHaveAttribute('open');
	});

	it('reports "action" from a closing button after its own handler', async () => {
		const order: string[] = [];
		const onClose = vi.fn(() => order.push('close'));
		render(
			<Dialog
				open
				title="Done"
				onClose={onClose}
				footer={
					<DialogActions>
						<DialogButton closes onClick={() => order.push('click')}>
							OK
						</DialogButton>
					</DialogActions>
				}
			/>,
		);
		await userEvent.click(screen.getByRole('button', { name: 'OK' }));
		expect(onClose).toHaveBeenCalledWith('action');
		expect(order).toEqual(['click', 'close']);
	});
});

describe('Dialog focus', () => {
	it('focuses the first field by default', () => {
		renderDialog();
		expect(screen.getByLabelText('Name')).toHaveFocus();
	});

	it('focuses the primary button when there is no field and no danger button', () => {
		render(
			<Dialog
				open
				title="Done"
				onClose={() => {}}
				footer={
					<DialogActions>
						<DialogButton>Later</DialogButton>
						<DialogButton variant="primary">Go</DialogButton>
					</DialogActions>
				}
			/>,
		);
		expect(screen.getByRole('button', { name: 'Go' })).toHaveFocus();
	});

	it('never focuses a danger button; the secondary one wins', () => {
		render(
			<Dialog
				open
				title="Delete"
				onClose={() => {}}
				footer={
					<DialogActions>
						<DialogButton variant="danger">Delete</DialogButton>
						<DialogButton variant="secondary">Keep</DialogButton>
					</DialogActions>
				}
			/>,
		);
		expect(screen.getByRole('button', { name: 'Keep' })).toHaveFocus();
	});

	it('focuses the dialog surface when only a danger button exists', () => {
		render(
			<Dialog
				open
				title="Delete"
				onClose={() => {}}
				footer={
					<DialogActions>
						<DialogButton variant="danger">Delete</DialogButton>
					</DialogActions>
				}
			/>,
		);
		expect(screen.getByRole('button', { name: 'Delete' })).not.toHaveFocus();
		expect(document.querySelector('dialog')?.contains(document.activeElement)).toBe(true);
	});

	it('honours an initialFocus selector and a ref', () => {
		const first = renderDialog({ initialFocus: 'input[aria-label="Extension"]' });
		expect(screen.getByLabelText('Extension')).toHaveFocus();
		first.unmount();
		const ref = createRef<HTMLInputElement>();
		render(
			<Dialog open title="T" onClose={() => {}} initialFocus={ref}>
				<input aria-label="A" />
				<input aria-label="B" ref={ref} />
			</Dialog>,
		);
		expect(screen.getByLabelText('B')).toHaveFocus();
	});

	it('restores focus to the opener on close', () => {
		function Host() {
			const [open, setOpen] = useState(false);
			return (
				<>
					<button onClick={() => setOpen(true)}>Open</button>
					<Dialog open={open} title="T" onClose={() => setOpen(false)}>
						<input aria-label="Field" />
					</Dialog>
				</>
			);
		}
		render(<Host />);
		const opener = screen.getByRole('button', { name: 'Open' });
		opener.focus();
		fireEvent.click(opener);
		expect(screen.getByLabelText('Field')).toHaveFocus();
		fireEvent.keyDown(document, { key: 'Escape' });
		expect(document.querySelector('dialog')).toBeNull();
		expect(opener).toHaveFocus();
	});

	it('restores focus to returnFocusTo when given', () => {
		const target = document.createElement('button');
		document.body.append(target);
		const { rerender } = renderDialog({ returnFocusTo: target });
		rerender(<Dialog open={false} title="T" onClose={() => {}} returnFocusTo={target} />);
		expect(target).toHaveFocus();
		target.remove();
	});

	it('keeps Tab inside the dialog and wraps both ways', async () => {
		render(
			<>
				<button>Outside</button>
				<Dialog open title="T" onClose={() => {}}>
					<input aria-label="One" />
					<input aria-label="Two" />
				</Dialog>
			</>,
		);
		const user = userEvent.setup();
		expect(screen.getByLabelText('One')).toHaveFocus();
		await user.tab();
		expect(screen.getByLabelText('Two')).toHaveFocus();
		await user.tab();
		expect(screen.getByLabelText('One')).toHaveFocus();
		await user.tab({ shift: true });
		expect(screen.getByLabelText('Two')).toHaveFocus();
	});

	it('pulls focus back in when it was outside', () => {
		render(
			<>
				<button>Outside</button>
				<Dialog open title="T" onClose={() => {}}>
					<input aria-label="One" />
				</Dialog>
			</>,
		);
		screen.getByRole('button', { name: 'Outside' }).focus();
		fireEvent.keyDown(screen.getByLabelText('One'), { key: 'Tab' });
		fireEvent.keyDown(document.querySelector('dialog') as HTMLElement, { key: 'Tab' });
		expect(screen.getByLabelText('One')).toHaveFocus();
	});
});

describe('Dialog stacking', () => {
	function Stack({ onInner, onOuter }: { onInner: () => void; onOuter: () => void }) {
		return (
			<Dialog open title="Outer" onClose={onOuter}>
				<input aria-label="Outer field" />
				<Dialog open title="Inner" onClose={onInner}>
					<input aria-label="Inner field" />
				</Dialog>
			</Dialog>
		);
	}

	it('answers Esc only on the top dialog', async () => {
		const onInner = vi.fn();
		const onOuter = vi.fn();
		render(<Stack onInner={onInner} onOuter={onOuter} />);
		await userEvent.keyboard('{Escape}');
		expect(onInner).toHaveBeenCalledExactlyOnceWith('escape');
		expect(onOuter).not.toHaveBeenCalled();
	});

	it('traps Tab in the inner dialog and restores focus to the outer one on close', async () => {
		const user = userEvent.setup();
		const { rerender } = render(<Stack onInner={() => {}} onOuter={() => {}} />);
		expect(screen.getByLabelText('Inner field')).toHaveFocus();
		await user.tab();
		expect(screen.getByLabelText('Inner field')).toHaveFocus();
		rerender(
			<Dialog open title="Outer" onClose={() => {}}>
				<input aria-label="Outer field" />
			</Dialog>,
		);
		expect(document.querySelector('dialog[open]')?.contains(document.activeElement)).toBe(true);
	});
});

describe('Dialog motion', () => {
	it('adds the reduced-motion class when the user prefers it', () => {
		const original = globalThis.matchMedia;
		globalThis.matchMedia = ((query: string) => ({
			matches: query.includes('prefers-reduced-motion'),
			media: query,
			addEventListener: () => {},
			removeEventListener: () => {},
		})) as unknown as typeof globalThis.matchMedia;
		try {
			renderDialog();
			expect(document.querySelector('dialog')?.className).toMatch(/reducedMotion/);
		} finally {
			globalThis.matchMedia = original;
		}
	});

	it('leaves the class off otherwise', () => {
		const original = globalThis.matchMedia;
		globalThis.matchMedia = ((query: string) => ({
			matches: false,
			media: query,
			addEventListener: () => {},
			removeEventListener: () => {},
		})) as unknown as typeof globalThis.matchMedia;
		try {
			renderDialog();
			expect(document.querySelector('dialog')?.className).not.toMatch(/reducedMotion/);
		} finally {
			globalThis.matchMedia = original;
		}
	});
});

describe('ConfirmDialog', () => {
	function confirm(props: Partial<Parameters<typeof ConfirmDialog>[0]> = {}) {
		const onConfirm = vi.fn();
		const onCancel = vi.fn();
		render(
			<ConfirmDialog
				open
				title="Delete 3 items"
				message="They go to the Trash."
				confirmLabel="Delete"
				onConfirm={onConfirm}
				onCancel={onCancel}
				{...props}
			/>,
		);
		return { onConfirm, onCancel };
	}

	it('names, describes and labels the buttons', () => {
		confirm();
		const dialog = screen.getByRole('dialog', { name: 'Delete 3 items' });
		expect(dialog).toHaveAccessibleDescription('They go to the Trash.');
		expect(screen.getByRole('button', { name: 'Cancel' })).toBeInTheDocument();
	});

	it('focuses the confirm button when it is not dangerous', () => {
		confirm();
		expect(screen.getByRole('button', { name: 'Delete' })).toHaveFocus();
	});

	it('focuses Cancel when dangerous', () => {
		confirm({ danger: true });
		expect(screen.getByRole('button', { name: 'Cancel' })).toHaveFocus();
		expect(screen.getByRole('button', { name: 'Delete' })).toHaveAttribute(
			'data-variant',
			'danger',
		);
	});

	it('confirms and cancels', async () => {
		const { onConfirm, onCancel } = confirm({ danger: true });
		await userEvent.click(screen.getByRole('button', { name: 'Delete' }));
		expect(onConfirm).toHaveBeenCalledOnce();
		await userEvent.click(screen.getByRole('button', { name: 'Cancel' }));
		expect(onCancel).toHaveBeenCalledOnce();
	});

	it('treats Esc as cancel', async () => {
		const { onCancel, onConfirm } = confirm();
		await userEvent.keyboard('{Escape}');
		expect(onCancel).toHaveBeenCalledOnce();
		expect(onConfirm).not.toHaveBeenCalled();
	});
});
