// Tests for the Ctrl+F2 shortcut that opens the batch rename dialog
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Sources } from '@liminal-hq/waypoint-protocol/generated/Sources';
import { cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { BatchRenameSelection } from './batchRenameApi';
import { isBatchRenameKey, useBatchRenameShortcut } from './useBatchRenameShortcut';

afterEach(cleanup);

const sources: Sources = { kind: 'locations', locations: [] };

function Probe(props: {
	selection: () => BatchRenameSelection | null;
	open: (selection: BatchRenameSelection) => void;
}) {
	useBatchRenameShortcut(props.selection, props.open);
	return null;
}

function press(init: KeyboardEventInit) {
	const event = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init });
	window.dispatchEvent(event);
	return event;
}

describe('isBatchRenameKey', () => {
	it('is Ctrl+F2 and Cmd+F2 with nothing else held', () => {
		const key = (init: KeyboardEventInit) => isBatchRenameKey(new KeyboardEvent('keydown', init));
		expect(key({ key: 'F2', ctrlKey: true })).toBe(true);
		expect(key({ key: 'F2', metaKey: true })).toBe(true);
		expect(key({ key: 'F2' })).toBe(false);
		expect(key({ key: 'F2', ctrlKey: true, shiftKey: true })).toBe(false);
		expect(key({ key: 'F2', ctrlKey: true, altKey: true })).toBe(false);
		expect(key({ key: 'F3', ctrlKey: true })).toBe(false);
	});
});

describe('useBatchRenameShortcut', () => {
	it('opens the dialog for the current selection and consumes the key', () => {
		const open = vi.fn();
		render(<Probe selection={() => ({ sources, count: 2 })} open={open} />);
		const event = press({ key: 'F2', ctrlKey: true });
		expect(open).toHaveBeenCalledWith({ sources, count: 2 });
		expect(event.defaultPrevented).toBe(true);
	});

	it('does nothing, and leaves the key alone, with nothing selected', () => {
		const open = vi.fn();
		render(<Probe selection={() => null} open={open} />);
		const event = press({ key: 'F2', ctrlKey: true });
		expect(open).not.toHaveBeenCalled();
		expect(event.defaultPrevented).toBe(false);
	});

	it('ignores a key something else has handled, and plain F2', () => {
		const open = vi.fn();
		const handled = (e: KeyboardEvent) => e.preventDefault();
		window.addEventListener('keydown', handled, { once: true });
		render(<Probe selection={() => ({ sources })} open={open} />);
		press({ key: 'F2', ctrlKey: true });
		press({ key: 'F2' });
		expect(open).not.toHaveBeenCalled();
	});

	it('stops listening when it unmounts', () => {
		const open = vi.fn();
		const { unmount } = render(<Probe selection={() => ({ sources })} open={open} />);
		unmount();
		fireEvent.keyDown(window, { key: 'F2', ctrlKey: true });
		expect(open).not.toHaveBeenCalled();
	});
});
