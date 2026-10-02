// Verifies the generic drag session: threshold, capture, Esc, cancel, holds, motion, store and live region
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { fireEvent } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
	createDragSession,
	motionMs,
	type DragClock,
	type DragHandlers,
	type DragSession,
} from './dragSession';

/** A clock the test winds by hand, so holds need no waiting. */
class ManualClock implements DragClock {
	private now = 0;
	private next = 1;
	private timers = new Map<number, { at: number; handler: () => void }>();
	setTimeout(handler: () => void, ms: number) {
		const id = this.next++;
		this.timers.set(id, { at: this.now + ms, handler });
		return id;
	}
	clearTimeout(handle: unknown) {
		this.timers.delete(handle as number);
	}
	advance(ms: number) {
		const end = this.now + ms;
		for (;;) {
			const due = [...this.timers.entries()]
				.filter(([, timer]) => timer.at <= end)
				.sort((a, b) => a[1].at - b[1].at)[0];
			if (!due) break;
			this.timers.delete(due[0]);
			this.now = due[1].at;
			due[1].handler();
		}
		this.now = end;
	}
}

type Source = { name: string };
type Target = { name: string };

function setup(reduced = false) {
	const clock = new ManualClock();
	const announce = vi.fn();
	const session: DragSession<Source, Target> = createDragSession<Source, Target>({
		startThresholdPx: 4,
		announce,
		clock,
		reducedMotion: () => reduced,
	});
	const handlers = {
		start: vi.fn(),
		move: vi.fn(),
		drop: vi.fn(),
		cancel: vi.fn(),
	} satisfies DragHandlers<Source, Target>;
	const element = document.createElement('div');
	document.body.append(element);
	element.setPointerCapture = vi.fn();
	element.releasePointerCapture = vi.fn();
	const begin = (x = 50, y = 10) =>
		session.begin(
			{ pointerId: 1, clientX: x, clientY: y, element, source: { name: 'tab' } },
			handlers,
		);
	const move = (x: number, y = 10) =>
		fireEvent.pointerMove(window, { clientX: x, clientY: y, pointerId: 1 });
	const up = (x: number, y = 10) =>
		fireEvent.pointerUp(window, { clientX: x, clientY: y, pointerId: 1 });
	return { clock, announce, session, handlers, element, begin, move, up };
}

afterEach(() => {
	document.body.innerHTML = '';
	document.documentElement.removeAttribute('style');
});

describe('a press', () => {
	it('is pending until the pointer has moved the start threshold, then a drag', () => {
		const { session, handlers, begin, move } = setup();
		expect(begin()).toBe(true);
		expect(session.store.getState().phase).toBe('pending');
		move(52);
		expect(session.store.getState().phase).toBe('pending');
		expect(handlers.move).not.toHaveBeenCalled();
		move(54);
		expect(session.store.getState().phase).toBe('dragging');
		expect(handlers.start).toHaveBeenCalledTimes(1);
		expect(handlers.move).toHaveBeenCalledTimes(1);
		expect(session.store.getState().source).toEqual({ name: 'tab' });
	});

	it('counts the threshold in both directions', () => {
		const { session, begin, move } = setup();
		begin(50, 10);
		move(50, 13);
		expect(session.store.getState().phase).toBe('pending');
		move(50, 14);
		expect(session.store.getState().phase).toBe('dragging');
	});

	it('that never becomes a drag is a click: no drop, no suppressed click, back to idle', () => {
		const { session, handlers, begin, move, up } = setup();
		begin();
		move(52);
		up(52);
		expect(handlers.drop).not.toHaveBeenCalled();
		expect(session.store.getState().phase).toBe('idle');
		expect(session.consumeClick()).toBe(false);
	});

	it('captures the pointer on the source only once it is a drag, and lets go at the end', () => {
		const { element, begin, move, up } = setup();
		begin();
		expect(element.setPointerCapture).not.toHaveBeenCalled();
		move(60);
		expect(element.setPointerCapture).toHaveBeenCalledWith(1);
		up(60);
		expect(element.releasePointerCapture).toHaveBeenCalledWith(1);
	});

	it('ignores another pointer, and refuses a second drag while one runs', () => {
		const { session, handlers, begin } = setup();
		begin();
		expect(begin()).toBe(false);
		fireEvent.pointerMove(window, { clientX: 99, clientY: 10, pointerId: 7 });
		expect(session.store.getState().phase).toBe('pending');
		expect(handlers.move).not.toHaveBeenCalled();
	});
});

describe('a drag', () => {
	it('writes the pointer and its travel to custom properties without touching the store', () => {
		const { session, begin, move } = setup();
		begin(50, 10);
		move(60, 12);
		const style = document.documentElement.style;
		expect(style.getPropertyValue('--wp-drag-x')).toBe('60px');
		expect(style.getPropertyValue('--wp-drag-y')).toBe('12px');
		expect(style.getPropertyValue('--wp-drag-dx')).toBe('10px');
		expect(style.getPropertyValue('--wp-drag-dy')).toBe('2px');
		const listener = vi.fn();
		const stop = session.store.subscribe(listener);
		move(80, 12);
		expect(listener).not.toHaveBeenCalled();
		stop();
	});

	it('drops on release at the release point, and the click that follows is suppressed once', () => {
		const { session, handlers, clock, begin, move, up } = setup();
		begin();
		move(60);
		up(90);
		expect(handlers.drop).toHaveBeenCalledTimes(1);
		expect(handlers.drop.mock.calls[0]![1]).toEqual({ x: 90, y: 10 });
		expect(session.store.getState().phase).toBe('dropped');
		expect(session.consumeClick()).toBe(true);
		expect(session.consumeClick()).toBe(false);
		expect(document.documentElement.style.getPropertyValue('--wp-drag-x')).toBe('');
		clock.advance(140);
		expect(session.store.getState().phase).toBe('idle');
	});

	it('drops the click flag when no click follows', () => {
		const { session, clock, begin, move, up } = setup();
		begin();
		move(60);
		up(60);
		clock.advance(0);
		expect(session.consumeClick()).toBe(false);
	});

	it('is abandoned by Escape, which nothing else sees', () => {
		const { session, handlers, announce, begin, move, up } = setup();
		const seen = vi.fn();
		document.addEventListener('keydown', seen);
		begin();
		move(60);
		fireEvent.keyDown(document.body, { key: 'Escape' });
		expect(handlers.cancel).toHaveBeenCalledWith(expect.anything(), 'escape');
		expect(session.store.getState().phase).toBe('cancelled');
		expect(seen).not.toHaveBeenCalled();
		up(60);
		expect(handlers.drop).not.toHaveBeenCalled();
		expect(session.consumeClick()).toBe(true);
		expect(announce).not.toHaveBeenCalled();
		document.removeEventListener('keydown', seen);
	});

	it('lets Escape through while a press has not become a drag', () => {
		const { session, begin } = setup();
		const seen = vi.fn();
		document.addEventListener('keydown', seen);
		begin();
		fireEvent.keyDown(document.body, { key: 'Escape' });
		expect(seen).toHaveBeenCalled();
		expect(session.store.getState().phase).toBe('pending');
		document.removeEventListener('keydown', seen);
	});

	it('is cancelled when the browser takes the pointer', () => {
		const { session, handlers, begin, move } = setup();
		begin();
		move(60);
		fireEvent.pointerCancel(window, { pointerId: 1 });
		expect(handlers.cancel).toHaveBeenCalledWith(expect.anything(), 'pointercancel');
		expect(session.store.getState().phase).toBe('cancelled');
	});

	it('is cancelled by cancel(), and a pending press by it simply ends', () => {
		const first = setup();
		first.begin();
		first.move(60);
		first.session.cancel();
		expect(first.handlers.cancel).toHaveBeenCalledWith(expect.anything(), 'api');
		const second = setup();
		second.begin();
		second.session.cancel();
		expect(second.handlers.cancel).not.toHaveBeenCalled();
		expect(second.session.store.getState().phase).toBe('idle');
	});

	it('survives a drop that throws or rejects', async () => {
		const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
		const thrown = setup();
		thrown.handlers.drop.mockImplementation(() => {
			throw new Error('no');
		});
		thrown.begin();
		thrown.move(60);
		thrown.up(60);
		const rejected = setup();
		rejected.handlers.drop.mockImplementation(() => Promise.reject(new Error('no')));
		rejected.begin();
		rejected.move(60);
		rejected.up(60);
		await Promise.resolve();
		await Promise.resolve();
		expect(warn).toHaveBeenCalledTimes(2);
		warn.mockRestore();
	});

	it('lets a new drag start as soon as the last one has ended, even mid-settle', () => {
		const { session, begin, move, up } = setup();
		begin();
		move(60);
		up(60);
		expect(session.store.getState().phase).toBe('dropped');
		expect(begin()).toBe(true);
		expect(session.store.getState().phase).toBe('pending');
	});
});

describe('holds', () => {
	it('fire after their time while the drag runs, and a second hold replaces the first', () => {
		const { session, clock, handlers, begin, move } = setup();
		const first = vi.fn();
		const second = vi.fn();
		handlers.move.mockImplementation((control) => {
			control.hold('rest', 450, first);
			control.hold('rest', 450, second);
		});
		begin();
		move(60);
		clock.advance(449);
		expect(second).not.toHaveBeenCalled();
		clock.advance(1);
		expect(first).not.toHaveBeenCalled();
		expect(second).toHaveBeenCalledTimes(1);
		expect(session.store.getState().phase).toBe('dragging');
	});

	it('can be cleared, and never fire after the drag ends', () => {
		const { clock, handlers, begin, move, up } = setup();
		const kept = vi.fn();
		const cleared = vi.fn();
		handlers.move.mockImplementation((control) => {
			control.hold('a', 100, cleared);
			control.clearHold('a');
			control.hold('b', 100, kept);
		});
		begin();
		move(60);
		up(60);
		clock.advance(1000);
		expect(cleared).not.toHaveBeenCalled();
		expect(kept).not.toHaveBeenCalled();
	});
});

describe('the store and the live region', () => {
	it('keeps the target and pill, announcing a pill only when its text changes', () => {
		const { session, announce, handlers, begin, move } = setup();
		handlers.move.mockImplementation((control, point) => {
			control.setTarget({ name: `at ${point.x}` });
			control.setPill({
				kind: 'move',
				text: point.x < 70 ? 'Release to move' : 'Release to add',
				announce: point.x < 70 ? 'Drag: release to move' : 'Drag: release to add',
			});
		});
		begin();
		move(60);
		move(62);
		expect(announce).toHaveBeenCalledTimes(1);
		expect(announce).toHaveBeenLastCalledWith('Drag: release to move');
		move(80);
		expect(announce).toHaveBeenLastCalledWith('Drag: release to add');
		expect(session.store.getState().target).toEqual({ name: 'at 80' });
		expect(session.store.getState().pill?.text).toBe('Release to add');
	});

	it('does not re-render for a pill or target that has not changed', () => {
		const { session, handlers, begin, move } = setup();
		handlers.move.mockImplementation((control) => {
			control.setPill({ kind: 'move', text: 'Same' });
		});
		begin();
		move(60);
		const listener = vi.fn();
		session.store.subscribe(listener);
		move(70);
		expect(listener).not.toHaveBeenCalled();
	});

	it('clears the pill and, after a motion, everything on drop', () => {
		const { session, clock, handlers, begin, move, up } = setup();
		handlers.move.mockImplementation((control) => {
			control.setTarget({ name: 'x' });
			control.setPill({ kind: 'move', text: 'Release' });
		});
		begin();
		move(60);
		up(60);
		const state = session.store.getState();
		expect(state.pill).toBeNull();
		expect(state.target).toEqual({ name: 'x' });
		clock.advance(140);
		expect(session.store.getState()).toEqual({
			phase: 'idle',
			pill: null,
			target: null,
			source: null,
		});
	});
});

describe('reduced motion', () => {
	it('settles at once instead of after 140 ms', () => {
		const { session, begin, move, up } = setup(true);
		expect(session.motionMs()).toBe(0);
		begin();
		move(60);
		up(60);
		expect(session.store.getState().phase).toBe('idle');
	});

	it('reads the system setting', () => {
		const original = globalThis.matchMedia;
		try {
			globalThis.matchMedia = ((query: string) => ({
				matches: query.includes('prefers-reduced-motion'),
			})) as typeof matchMedia;
			expect(motionMs()).toBe(0);
			globalThis.matchMedia = (() => ({ matches: false })) as unknown as typeof matchMedia;
			expect(motionMs()).toBe(140);
		} finally {
			globalThis.matchMedia = original;
		}
	});
});

describe('a drag the system runs (beginExternal)', () => {
	const start = (session: DragSession<Source, Target>, handlers: DragHandlers<Source, Target>) =>
		session.beginExternal({ point: { x: 20, y: 30 }, source: { name: 'files' } }, handlers);

	it('is dragging at once, with no press and no threshold, and tells the owner where the files are', () => {
		const { session, handlers } = setup();
		const drag = start(session, handlers)!;
		expect(session.store.getState()).toMatchObject({
			phase: 'dragging',
			source: { name: 'files' },
		});
		expect(handlers.start).toHaveBeenCalledTimes(1);
		expect(handlers.move).toHaveBeenCalledWith(expect.anything(), { x: 20, y: 30 });
		expect(document.documentElement.style.getPropertyValue('--wp-drag-x')).toBe('20px');
		drag.move({ x: 90, y: 40 });
		expect(handlers.move).toHaveBeenLastCalledWith(expect.anything(), { x: 90, y: 40 });
		expect(document.documentElement.style.getPropertyValue('--wp-drag-dx')).toBe('70px');
		expect(document.documentElement.style.getPropertyValue('--wp-drag-dy')).toBe('10px');
	});

	it('refuses to begin while a drag runs', () => {
		const { session, handlers, begin, move } = setup();
		begin();
		move(60);
		expect(start(session, handlers)).toBeNull();
		session.cancel();
		const first = start(session, handlers);
		expect(first).not.toBeNull();
		expect(start(session, handlers)).toBeNull();
	});

	it('drops with a last look at where it ended, then settles', () => {
		const { session, handlers, clock } = setup();
		const drag = start(session, handlers)!;
		drag.drop({ x: 55, y: 66 });
		expect(handlers.move).toHaveBeenLastCalledWith(expect.anything(), { x: 55, y: 66 });
		expect(handlers.drop).toHaveBeenCalledWith(expect.anything(), { x: 55, y: 66 });
		expect(session.store.getState().phase).toBe('dropped');
		clock.advance(140);
		expect(session.store.getState().phase).toBe('idle');
		// Once dropped it takes no more.
		drag.move({ x: 1, y: 1 });
		drag.drop({ x: 1, y: 1 });
		expect(handlers.drop).toHaveBeenCalledTimes(1);
	});

	it('cancels with the reason it is given, and takes no more positions', () => {
		const { session, handlers } = setup();
		const drag = start(session, handlers)!;
		drag.cancel('left');
		expect(handlers.cancel).toHaveBeenCalledWith(expect.anything(), 'left');
		expect(session.store.getState().phase).toBe('cancelled');
		const moves = handlers.move.mock.calls.length;
		drag.move({ x: 1, y: 1 });
		expect(handlers.move).toHaveBeenCalledTimes(moves);
	});

	it('is ended by Esc where the page hears it, and a stale feed does nothing to the next drag', () => {
		const { session, handlers } = setup();
		const stale = start(session, handlers)!;
		fireEvent.keyDown(window, { key: 'Escape' });
		expect(handlers.cancel).toHaveBeenCalledWith(expect.anything(), 'escape');
		const fresh = start(session, handlers)!;
		stale.cancel();
		stale.drop({ x: 1, y: 1 });
		expect(session.store.getState().phase).toBe('dragging');
		fresh.cancel();
	});

	it('holds, like any drag, only while it is dragging', () => {
		const { session, handlers, clock } = setup();
		const drag = start(session, handlers)!;
		const fn = vi.fn();
		handlers.move.mockImplementation((control) => control.hold('x', 100, fn));
		drag.move({ x: 2, y: 2 });
		clock.advance(100);
		expect(fn).toHaveBeenCalledTimes(1);
	});
});
