// The divider on a list column heading that resizes the column by drag, by keyboard and by double-click
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import {
	useEffect,
	useLayoutEffect,
	useRef,
	useState,
	type KeyboardEvent,
	type PointerEvent,
} from 'react';
import { tf } from '../i18n/messages';
import styles from './ColumnResizeHandle.module.css';
import { COLUMN_LIMITS, widthForDrag, widthForKey, type ResizableColumn } from './columnWidths';

/** What Name may not be squeezed below when none is declared. */
const DEFAULT_NAME_MIN = 120;

interface ColumnResizeHandleProps {
	column: ResizableColumn;
	/** The column's heading, already translated: it names the divider. */
	label: string;
	/** The width in force for the column, when it has been resized. */
	width: number | undefined;
	/** Shows a width while it is being chosen; `undefined` drops it and `null` shows the column's own. */
	onPreview: (width: number | null | undefined) => void;
	/** Keeps the width, or `null` for the column's own. */
	onCommit: (width: number | null) => void;
	/** Keeps the width once the keys have settled. */
	onCommitSoon: (width: number | null) => void;
	/** Keeps any width still waiting on the keys. */
	onFlush: () => void;
}

interface Drag {
	pointerId: number;
	startX: number;
	from: number;
	rtl: boolean;
	room: number;
	last: number | null;
}

/**
 * The column's start edge: the divider follows the pointer because the columns after Name keep their
 * end edges and Name takes up the slack. Rust keeps the width; this reports it.
 */
export function ColumnResizeHandle({
	column,
	label,
	width,
	onPreview,
	onCommit,
	onCommitSoon,
	onFlush,
}: ColumnResizeHandleProps) {
	const element = useRef<HTMLDivElement | null>(null);
	const drag = useRef<Drag | null>(null);
	const [dragging, setDragging] = useState(false);
	const [measured, setMeasured] = useState<number | null>(null);
	const limits = COLUMN_LIMITS[column];
	const previewRef = useRef(onPreview);
	previewRef.current = onPreview;

	const read = () => {
		const size = element.current?.parentElement?.getBoundingClientRect().width ?? 0;
		setMeasured(size > 0 ? Math.round(size) : null);
	};
	// What the column measures now, which differs from the width in force while the view is too narrow for it.
	// Read after every render (a bailout when it is the same) and whenever the view resizes.
	useLayoutEffect(read);
	useLayoutEffect(() => {
		const cell = element.current?.parentElement;
		if (!cell || typeof ResizeObserver === 'undefined') return;
		const observer = new ResizeObserver(read);
		observer.observe(cell);
		return () => observer.disconnect();
	}, []);

	const current = measured ?? width ?? limits.initial;

	const measure = () => {
		const cell = element.current?.parentElement;
		const size = cell?.getBoundingClientRect().width ?? 0;
		return size > 0 ? size : (width ?? limits.initial);
	};
	// How much wider the column may grow before Name would shrink below its own minimum.
	const roomFromName = () => {
		const header = element.current?.parentElement?.parentElement;
		const name = header?.querySelector<HTMLElement>('[data-column="name"]');
		const nameWidth = name?.getBoundingClientRect().width ?? 0;
		if (!header || nameWidth <= 0) return Number.POSITIVE_INFINITY;
		const declared = Number.parseFloat(getComputedStyle(header).getPropertyValue('--wp-name-min'));
		return nameWidth - (Number.isFinite(declared) ? declared : DEFAULT_NAME_MIN);
	};
	const isRtl = (target: HTMLElement) => getComputedStyle(target).direction === 'rtl';

	const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
		if (event.button !== 0) return;
		event.currentTarget.setPointerCapture?.(event.pointerId);
		event.preventDefault();
		event.currentTarget.focus();
		drag.current = {
			pointerId: event.pointerId,
			startX: event.clientX,
			from: measure(),
			rtl: isRtl(event.currentTarget),
			room: roomFromName(),
			last: null,
		};
		setDragging(true);
	};
	const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
		const state = drag.current;
		if (!state) return;
		state.last = widthForDrag(column, state.from, event.clientX - state.startX, state);
		onPreview(state.last);
	};
	const finish = (event: PointerEvent<HTMLDivElement>, keep: boolean) => {
		const state = drag.current;
		if (!state) return;
		drag.current = null;
		setDragging(false);
		event.currentTarget.releasePointerCapture?.(state.pointerId);
		if (keep && state.last !== null) onCommit(state.last);
		else onPreview(undefined);
	};
	const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
		const next = widthForKey(column, measure(), event.key, {
			rtl: isRtl(event.currentTarget),
			shift: event.shiftKey,
			room: roomFromName(),
		});
		if (!next) return;
		event.preventDefault();
		event.stopPropagation();
		const target = next.kind === 'reset' ? null : next.width;
		if (target === null) onCommit(null);
		else onCommitSoon(target);
	};

	useEffect(() => {
		const onKey = (event: globalThis.KeyboardEvent) => {
			if (event.key !== 'Escape' || !drag.current) return;
			event.stopPropagation();
			drag.current = null;
			setDragging(false);
			previewRef.current(undefined);
		};
		window.addEventListener('keydown', onKey, true);
		return () => window.removeEventListener('keydown', onKey, true);
	}, []);

	return (
		<div
			ref={element}
			role="separator"
			tabIndex={0}
			className={styles.handle}
			aria-orientation="vertical"
			aria-label={tf('browse.column.resize', { column: label })}
			aria-valuenow={current}
			aria-valuemin={limits.min}
			aria-valuemax={limits.max}
			aria-valuetext={tf('browse.column.resize.value', { width: current })}
			data-dragging={dragging ? '' : undefined}
			onPointerDown={onPointerDown}
			onPointerMove={onPointerMove}
			onPointerUp={(event) => finish(event, true)}
			onPointerCancel={(event) => finish(event, false)}
			onDoubleClick={() => onCommit(null)}
			onKeyDown={onKeyDown}
			onBlur={onFlush}
		/>
	);
}
