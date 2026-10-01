// Verifies the drag ghost: its stack, count, badge and pill, and that it is drawn only while a drag runs
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { ListingSession } from '../browse/useListingSession';
import { createDragSession } from './dragSession';
import { DragStack } from './DragStack';
import type { FileDragSource, FileDropTarget } from './fileDragModel';

afterEach(() => {
	cleanup();
	document.body.innerHTML = '';
});

const source = (over: Partial<FileDragSource> = {}): FileDragSource => ({
	session: {} as ListingSession,
	tab: 1,
	handle: 1,
	spec: { kind: 'some', ids: [1] },
	count: 1,
	name: 'a.txt',
	groups: ['document'],
	folder: { display: '/a', uri: 'file:///a' },
	readOnly: false,
	rightButton: false,
	...over,
});

/** A session in the dragging phase with `pill`, as the file drag leaves it. */
function dragging(over: Partial<FileDragSource>, pill: { text: string; kind: string } | null) {
	const session = createDragSession<FileDragSource, FileDropTarget>({
		startThresholdPx: 0,
		reducedMotion: () => true,
	});
	session.begin(
		{ pointerId: 1, clientX: 0, clientY: 0, element: null, source: source(over) },
		{
			start: (control) => control.setPill(pill),
			move: () => {},
			drop: () => {},
		},
	);
	act(() => {
		window.dispatchEvent(new PointerEvent('pointermove', { clientX: 5, clientY: 5, pointerId: 1 }));
	});
	return session;
}

describe('DragStack', () => {
	it('draws nothing before a drag', () => {
		const session = createDragSession<FileDragSource, FileDropTarget>({ startThresholdPx: 4 });
		render(<DragStack session={session} />);
		expect(document.querySelector('[data-drag-stack]')).toBeNull();
	});

	it('is hidden from assistive technology: the live region says the same words', () => {
		const session = dragging({}, { text: 'Move a.txt to Docs', kind: 'move' });
		render(<DragStack session={session} />);
		const ghost = document.querySelector('[data-drag-stack]')!;
		expect(ghost).toHaveAttribute('aria-hidden', 'true');
		expect(ghost.querySelector('[data-drag-pill]')).toHaveTextContent('Move a.txt to Docs');
		expect(ghost.querySelector('[data-drag-pill]')).toHaveTextContent('Esc to cancel');
	});

	it('stacks up to three icons and shows a count for more than one', () => {
		const one = dragging({}, { text: 'x', kind: 'idle' });
		const { unmount } = render(<DragStack session={one} />);
		expect(document.querySelectorAll('[data-drag-stack] svg[data-group]')).toHaveLength(1);
		expect(document.querySelector('[data-drag-stack]')?.textContent).not.toMatch(/\d/);
		one.cancel();
		unmount();

		const many = dragging(
			{ count: 7, name: null, groups: ['image', 'document', 'document'] },
			{ text: 'x', kind: 'idle' },
		);
		render(<DragStack session={many} />);
		expect(document.querySelectorAll('[data-drag-stack] svg[data-group]')).toHaveLength(3);
		expect(document.querySelector('[data-drag-stack]')).toHaveTextContent('7');
	});

	it('caps an enormous count', () => {
		const session = dragging({ count: 250000, name: null }, { text: 'x', kind: 'idle' });
		render(<DragStack session={session} />);
		expect(document.querySelector('[data-drag-stack]')).toHaveTextContent('999+');
	});

	it('shows a badge for what a release does, and none when nothing is targeted', () => {
		for (const kind of ['copy', 'move', 'link', 'trash', 'ask', 'blocked', 'open', 'pending']) {
			const session = dragging({}, { text: 'x', kind });
			const { unmount } = render(<DragStack session={session} />);
			expect(
				document.querySelector(`[data-drag-stack] span[data-kind="${kind}"]`),
				kind,
			).not.toBeNull();
			expect(
				document.querySelector('[data-drag-stack] svg:not([data-group])'),
				`${kind} badge`,
			).not.toBeNull();
			session.cancel();
			unmount();
		}
		const idle = dragging({}, { text: 'x', kind: 'idle' });
		render(<DragStack session={idle} />);
		expect(document.querySelector('[data-drag-stack] svg:not([data-group])')).toBeNull();
	});

	it('marks a refusal by its kind, which the stylesheet gives a dashed edge and a shake', () => {
		const session = dragging({}, { text: 'Not allowed: nope', kind: 'blocked' });
		render(<DragStack session={session} />);
		expect(document.querySelector('[data-drag-pill]')).toHaveAttribute('data-kind', 'blocked');
	});

	it('is gone once the drag ends', () => {
		const session = dragging({}, { text: 'x', kind: 'idle' });
		render(<DragStack session={session} />);
		act(() => session.cancel());
		expect(document.querySelector('[data-drag-stack]')).toBeNull();
	});
});
