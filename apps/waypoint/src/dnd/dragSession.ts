// A pointer drag as a plain state machine: threshold, capture, Esc, hold timers, motion and a store
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { prefersReducedMotion } from '../theme/motion';
import { createStore, type StoreApi } from 'zustand/vanilla';

/** How long drag motion lasts (D82); it is instant under Reduce motion. */
export const DRAG_MOTION_MS = 140;

/** The phases of a drag. `cancelled` and `dropped` last for one motion, so the settle can play. */
export type DragPhase = 'idle' | 'pending' | 'dragging' | 'cancelled' | 'dropped';

/** The one label shown beside the pointer. `announce` is what the live region says, when it differs. */
export interface DragPill {
	text: string;
	kind: string;
	announce?: string;
	/** The label is announced but not drawn here: something else (the tear-off ghost) already shows it. */
	undrawn?: boolean;
}

export interface DragState<Source, Target> {
	phase: DragPhase;
	pill: DragPill | null;
	/** What a release would act on right now; the shape is the owner's. */
	target: Target | null;
	/** What was grabbed and how it was measured; set when the drag begins and kept until it settles. */
	source: Source | null;
}

export interface Point {
	x: number;
	y: number;
}

/** Timers, injectable so tests can run holds without waiting. */
export interface DragClock {
	setTimeout(handler: () => void, ms: number): unknown;
	clearTimeout(handle: unknown): void;
}

const realClock: DragClock = {
	setTimeout: (handler, ms) => globalThis.setTimeout(handler, ms),
	clearTimeout: (handle) => globalThis.clearTimeout(handle as ReturnType<typeof setTimeout>),
};

/** `left`: an external drag (files from another window or application) left the window without a drop. */
export type CancelReason = 'escape' | 'pointercancel' | 'api' | 'left';

/** What a drag's owner can see and change while the drag runs. */
export interface DragControl<Source, Target> {
	readonly source: Source;
	readonly origin: Point;
	readonly point: Point;
	target(): Target | null;
	pill(): DragPill | null;
	setTarget(target: Target | null): void;
	/** Shows a pill (or none) and announces it when its text changes. */
	setPill(pill: DragPill | null): void;
	/** Runs `fn` after `ms` of the drag still going; a second hold under the same key replaces the first. */
	hold(key: string, ms: number, fn: () => void): void;
	clearHold(key: string): void;
	announce(text: string): void;
	/** 140 ms, or 0 under Reduce motion. */
	motionMs(): number;
}

/** What a kind of drag does. The session decides when; the handlers decide what it means. */
export interface DragHandlers<Source, Target> {
	/** Once, when the pointer first passes the start threshold. */
	start?(control: DragControl<Source, Target>, point: Point): void;
	/** On every pointer move once the drag has started. */
	move(control: DragControl<Source, Target>, point: Point): void;
	/** On release. The phase is already `dropped`; the work may be asynchronous. */
	drop(control: DragControl<Source, Target>, point: Point): void | Promise<void>;
	/** When the drag is abandoned: Esc, the browser taking the pointer, or `cancel()`. */
	cancel?(control: DragControl<Source, Target>, reason: CancelReason): void;
}

export interface DragStart<Source> {
	pointerId: number;
	clientX: number;
	clientY: number;
	/** The element that gets pointer capture; the session listens on the window either way. */
	element?: Element | null;
	/** What is being dragged, handed back through `control.source` and the store. */
	source: Source;
}

export interface DragSessionOptions<Target = unknown> {
	/** The pointer travels this far before a press becomes a drag. */
	startThresholdPx: number;
	/** The live region's feed. */
	announce?: (text: string) => void;
	clock?: DragClock;
	/** Whether the person asked for less motion; read when a drag settles. */
	reducedMotion?: () => boolean;
	/** Whether two targets are the same, so a steady hover does not re-render; `Object.is` by default. */
	sameTarget?: (a: Target, b: Target) => boolean;
	/** The element that carries the `--wp-drag-*` custom properties; the document element by default. */
	root?: () => HTMLElement | null;
}

/** The way in to a drag whose pointer the page does not see: the owner feeds it positions. */
export interface ExternalDrag {
	/** The drag is over `point`. */
	move(point: Point): void;
	/** Released at `point`. */
	drop(point: Point): void | Promise<void>;
	/** Left the window, or was abandoned. */
	cancel(reason?: CancelReason): void;
}

export interface DragSession<Source, Target> {
	readonly store: StoreApi<DragState<Source, Target>>;
	/** Starts tracking a press; false when a drag is already running. */
	begin(start: DragStart<Source>, handlers: DragHandlers<Source, Target>): boolean;
	/**
	 * Starts a drag that is already under way, one the system runs (files dragged in from another
	 * window or application): there is no press to track, so the owner reports where it is with the
	 * returned `ExternalDrag`. `null` when a drag is already running.
	 */
	beginExternal(
		start: { point: Point; source: Source },
		handlers: DragHandlers<Source, Target>,
	): ExternalDrag | null;
	/** Abandons the drag, if one is running. */
	cancel(): void;
	/** The click that follows a drag is not a click; true once after a drag ends, then false. */
	consumeClick(): boolean;
	motionMs(): number;
	dispose(): void;
}

/** 140 ms, or 0 when the system asks for reduced motion. */
export function motionMs(): number {
	return prefersReducedMotion() ? 0 : DRAG_MOTION_MS;
}

const IDLE: DragState<never, never> = { phase: 'idle', pill: null, target: null, source: null };

/**
 * The state of one pointer drag, with no knowledge of what is dragged. A press is `pending` until
 * the pointer has moved `startThresholdPx`, then `dragging` until release (`dropped`) or Esc or a
 * `pointercancel` (`cancelled`); either end stays for one motion before the store returns to
 * `idle`. The pointer's position is written to `--wp-drag-x`, `--wp-drag-y` (the pointer) and
 * `--wp-drag-dx`, `--wp-drag-dy` (how far it has moved) on the root element, so the thing that
 * follows the pointer moves without React rendering every move. The store holds only what changes
 * discretely: the phase, the pill and the target.
 */
export function createDragSession<Source, Target>(
	options: DragSessionOptions<Target>,
): DragSession<Source, Target> {
	const clock = options.clock ?? realClock;
	const reduced = options.reducedMotion ?? prefersReducedMotion;
	const store = createStore<DragState<Source, Target>>(() => ({
		...(IDLE as DragState<Source, Target>),
	}));
	const rootElement = () =>
		options.root
			? options.root()
			: typeof document === 'undefined'
				? null
				: document.documentElement;
	const ms = () => (reduced() ? 0 : DRAG_MOTION_MS);
	const say = (text: string) => options.announce?.(text);

	let handlers: DragHandlers<Source, Target> | null = null;
	let pointerId = -1;
	let capturing: Element | null = null;
	let captured: Element | null = null;
	let origin: Point = { x: 0, y: 0 };
	let point: Point = { x: 0, y: 0 };
	let suppressClick = false;
	// Which drag is running, so a feed from one that ended cannot steer the next.
	let serial = 0;
	let settleTimer: unknown;
	let clickTimer: unknown;
	const holds = new Map<string, unknown>();

	const control: DragControl<Source, Target> = {
		get source() {
			return store.getState().source as Source;
		},
		get origin() {
			return origin;
		},
		get point() {
			return point;
		},
		target: () => store.getState().target,
		pill: () => store.getState().pill,
		setTarget(target) {
			const previous = store.getState().target;
			const same = options.sameTarget ?? Object.is;
			if (previous === target || (previous && target && same(previous, target))) return;
			store.setState({ target });
		},
		setPill(pill) {
			const previous = store.getState().pill;
			if (previous === pill) return;
			if (previous && pill && previous.text === pill.text && previous.kind === pill.kind) return;
			store.setState({ pill });
			if (pill && (pill.text !== previous?.text || pill.kind !== previous?.kind)) {
				say(pill.announce ?? pill.text);
			}
		},
		hold(key, delay, fn) {
			control.clearHold(key);
			holds.set(
				key,
				clock.setTimeout(() => {
					holds.delete(key);
					if (store.getState().phase === 'dragging') fn();
				}, delay),
			);
		},
		clearHold(key) {
			if (!holds.has(key)) return;
			clock.clearTimeout(holds.get(key));
			holds.delete(key);
		},
		announce: say,
		motionMs: ms,
	};

	const setVariables = () => {
		const style = rootElement()?.style;
		if (!style) return;
		style.setProperty('--wp-drag-x', `${point.x}px`);
		style.setProperty('--wp-drag-y', `${point.y}px`);
		style.setProperty('--wp-drag-dx', `${point.x - origin.x}px`);
		style.setProperty('--wp-drag-dy', `${point.y - origin.y}px`);
	};
	const clearVariables = () => {
		const style = rootElement()?.style;
		for (const name of ['--wp-drag-x', '--wp-drag-y', '--wp-drag-dx', '--wp-drag-dy']) {
			style?.removeProperty(name);
		}
	};

	const stopListening = () => {
		window.removeEventListener('pointermove', onMove, true);
		window.removeEventListener('pointerup', onUp, true);
		window.removeEventListener('pointercancel', onPointerCancel, true);
		window.removeEventListener('keydown', onKey, true);
		if (captured) {
			try {
				captured.releasePointerCapture?.(pointerId);
			} catch {
				// The pointer is already gone, which is what releasing was for.
			}
			captured = null;
		}
	};

	const clearHolds = () => {
		for (const handle of holds.values()) clock.clearTimeout(handle);
		holds.clear();
	};

	/** Ends tracking and lets the settle play for one motion before the store goes idle. */
	const finish = (phase: 'dropped' | 'cancelled' | 'idle') => {
		clearHolds();
		stopListening();
		clearVariables();
		handlers = null;
		capturing = null;
		clock.clearTimeout(settleTimer);
		if (phase === 'idle') {
			store.setState({ ...(IDLE as DragState<Source, Target>) });
			return;
		}
		suppressClick = true;
		clock.clearTimeout(clickTimer);
		// The click that ends a drag arrives straight after the release; drop the flag if none does.
		clickTimer = clock.setTimeout(() => {
			suppressClick = false;
		}, 0);
		store.setState({ phase, pill: null });
		const settle = () => store.setState({ ...(IDLE as DragState<Source, Target>) });
		if (ms() === 0) settle();
		else settleTimer = clock.setTimeout(settle, ms());
	};

	const matches = (event: PointerEvent) =>
		typeof event.pointerId !== 'number' || pointerId < 0 || event.pointerId === pointerId;

	function onMove(event: PointerEvent) {
		if (!handlers || !matches(event)) return;
		point = { x: event.clientX, y: event.clientY };
		const phase = store.getState().phase;
		if (phase === 'pending') {
			if (Math.hypot(point.x - origin.x, point.y - origin.y) < options.startThresholdPx) return;
			store.setState({ phase: 'dragging' });
			// Captured only now: a press that stays a click must reach the element it landed on.
			try {
				capturing?.setPointerCapture?.(event.pointerId);
				captured = capturing;
			} catch {
				// A synthetic pointer cannot be captured; the window listeners still see it.
			}
			setVariables();
			handlers.start?.(control, point);
		} else if (phase === 'dragging') {
			setVariables();
		} else {
			return;
		}
		handlers?.move(control, point);
	}

	function onUp(event: PointerEvent) {
		if (!handlers || !matches(event)) return;
		const phase = store.getState().phase;
		if (phase !== 'dragging') {
			// A press that never became a drag is a click; leave it alone.
			finish('idle');
			return;
		}
		commit({ x: event.clientX, y: event.clientY });
	}

	/** The release of a drag that is running: one last look at where it ended, then the owner's drop. */
	function commit(next: Point): void | Promise<void> {
		const active = handlers;
		if (!active) return;
		if (next.x !== point.x || next.y !== point.y) {
			point = next;
			setVariables();
			active.move(control, point);
		}
		store.setState({ phase: 'dropped' });
		let outcome: void | Promise<void> = undefined;
		try {
			outcome = active.drop(control, point);
		} catch (error) {
			console.warn('drag drop failed', error);
		}
		if (outcome) {
			void outcome.catch((error: unknown) => console.warn('drag drop failed', error));
		}
		finish('dropped');
		return outcome;
	}

	function onPointerCancel(event: PointerEvent) {
		if (!handlers || !matches(event)) return;
		end('pointercancel');
	}

	function onKey(event: KeyboardEvent) {
		if (event.key !== 'Escape' || store.getState().phase !== 'dragging') return;
		event.preventDefault();
		event.stopPropagation();
		end('escape');
	}

	function end(reason: CancelReason) {
		if (!handlers) return;
		if (store.getState().phase !== 'dragging') {
			finish('idle');
			return;
		}
		const active = handlers;
		store.setState({ phase: 'cancelled' });
		active.cancel?.(control, reason);
		finish('cancelled');
	}

	return {
		store,
		begin(start, next) {
			const phase = store.getState().phase;
			if (phase === 'pending' || phase === 'dragging') return false;
			clock.clearTimeout(settleTimer);
			serial++;
			handlers = next;
			pointerId = start.pointerId;
			origin = { x: start.clientX, y: start.clientY };
			point = origin;
			capturing = start.element ?? null;
			store.setState({ phase: 'pending', pill: null, target: null, source: start.source });
			window.addEventListener('pointermove', onMove, true);
			window.addEventListener('pointerup', onUp, true);
			window.addEventListener('pointercancel', onPointerCancel, true);
			window.addEventListener('keydown', onKey, true);
			return true;
		},
		beginExternal(start, next) {
			const phase = store.getState().phase;
			if (phase === 'pending' || phase === 'dragging') return null;
			clock.clearTimeout(settleTimer);
			const mine = ++serial;
			handlers = next;
			pointerId = -1;
			capturing = null;
			origin = start.point;
			point = origin;
			store.setState({ phase: 'dragging', pill: null, target: null, source: start.source });
			// Esc reaches the page only where the system lets it; where it does, it ends the drag.
			window.addEventListener('keydown', onKey, true);
			setVariables();
			next.start?.(control, point);
			next.move(control, point);
			const live = next;
			const running = () =>
				serial === mine && handlers === live && store.getState().phase === 'dragging';
			return {
				move(to) {
					if (!running()) return;
					point = to;
					setVariables();
					live.move(control, point);
				},
				drop(to) {
					if (!running()) return;
					return commit(to);
				},
				cancel(reason = 'api') {
					if (running()) end(reason);
				},
			};
		},
		cancel() {
			if (!handlers) return;
			end('api');
		},
		consumeClick() {
			const was = suppressClick;
			suppressClick = false;
			return was;
		},
		motionMs: ms,
		dispose() {
			if (handlers) end('api');
			clock.clearTimeout(settleTimer);
			clock.clearTimeout(clickTimer);
		},
	};
}
