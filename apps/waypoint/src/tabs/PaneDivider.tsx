// The draggable, keyboard-resizable divider between two panes of a pair
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { PairLayout } from '@liminal-hq/waypoint-protocol/generated/PairLayout';
import { useEffect, useRef, type KeyboardEvent, type PointerEvent } from 'react';
import { t, tf } from '../i18n/messages';
import {
	DIVIDER_BIG_STEP,
	DIVIDER_STEP,
	MIN_PANE_SHARE,
	moveDivider,
	sameSizes,
} from './pairLayout';
import styles from './PaneDivider.module.css';

interface PaneDividerProps {
	layout: PairLayout;
	/** The divider sits after the pane at this index. */
	index: number;
	/** The sizes now (in thousandths), including a drag in progress. */
	sizes: readonly number[];
	/** Called while a drag moves, so the panes resize live and in memory only. */
	onResize: (sizes: number[]) => void;
	/** Called when a drag is released or a key moves the divider: the sizes to keep. */
	onCommit: (sizes: number[]) => void;
	/** Called when a drag is abandoned: go back to the sizes that were kept. */
	onCancel: () => void;
	/** Double-click, or Enter: equal sizes. */
	onReset: () => void;
}

interface Drag {
	start: number;
	sizes: readonly number[];
	/** Pixels the shares are spread over: the pair's area without its dividers. */
	extent: number;
	/** Pointer movement along the axis counts the other way in right-to-left text. */
	sign: 1 | -1;
	pointerId: number;
}

/**
 * A separator with `aria-valuenow` (the first pane's share, in percent). Arrow keys along the
 * layout's axis move it by 2% (10% with Shift), Enter resets to equal sizes, and the pointer drags
 * it. A drag resizes the panes in memory and writes the result when the pointer is released;
 * Escape abandons it.
 */
export function PaneDivider({
	layout,
	index,
	sizes,
	onResize,
	onCommit,
	onCancel,
	onReset,
}: PaneDividerProps) {
	const drag = useRef<Drag | null>(null);
	const horizontal = layout === 'sideBySide';
	const before = sizes[index] ?? 0;
	const after = sizes[index + 1] ?? 0;

	// Escape abandons a drag in progress.
	useEffect(() => {
		const onKey = (event: globalThis.KeyboardEvent) => {
			if (event.key !== 'Escape' || !drag.current) return;
			event.stopPropagation();
			drag.current = null;
			onCancel();
		};
		window.addEventListener('keydown', onKey, true);
		return () => window.removeEventListener('keydown', onKey, true);
	}, [onCancel]);

	const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
		if (event.button !== 0) return;
		const divider = event.currentTarget;
		const area = divider.parentElement;
		if (!area) return;
		const box = area.getBoundingClientRect();
		const thickness = divider.getBoundingClientRect()[horizontal ? 'width' : 'height'];
		// Only this area's own dividers: a list's column handles inside the panes are separators too.
		const dividers = area.querySelectorAll(':scope > [role="separator"]').length;
		const total = horizontal ? box.width : box.height;
		divider.setPointerCapture?.(event.pointerId);
		event.preventDefault();
		divider.focus();
		drag.current = {
			start: horizontal ? event.clientX : event.clientY,
			sizes,
			extent: Math.max(1, total - thickness * dividers),
			sign: horizontal && getComputedStyle(area).direction === 'rtl' ? -1 : 1,
			pointerId: event.pointerId,
		};
	};

	const delta = (event: PointerEvent, state: Drag): number =>
		Math.round(
			(((horizontal ? event.clientX : event.clientY) - state.start) * state.sign * 1000) /
				state.extent,
		);

	const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
		const state = drag.current;
		if (!state) return;
		onResize(moveDivider(state.sizes, index, delta(event, state)));
	};

	const finish = (event: PointerEvent<HTMLDivElement>, commit: boolean) => {
		const state = drag.current;
		if (!state) return;
		drag.current = null;
		event.currentTarget.releasePointerCapture?.(state.pointerId);
		if (!commit) return onCancel();
		const next = moveDivider(state.sizes, index, delta(event, state));
		if (sameSizes(next, state.sizes)) onCancel();
		else onCommit(next);
	};

	const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
		const rtl = horizontal && getComputedStyle(event.currentTarget).direction === 'rtl';
		const step = event.shiftKey ? DIVIDER_BIG_STEP : DIVIDER_STEP;
		let change = 0;
		if (horizontal && event.key === 'ArrowLeft') change = rtl ? step : -step;
		else if (horizontal && event.key === 'ArrowRight') change = rtl ? -step : step;
		else if (!horizontal && event.key === 'ArrowUp') change = -step;
		else if (!horizontal && event.key === 'ArrowDown') change = step;
		else if (event.key === 'Enter') {
			event.preventDefault();
			return onReset();
		} else return;
		event.preventDefault();
		const next = moveDivider(sizes, index, change);
		if (!sameSizes(next, sizes)) onCommit(next);
	};

	const percent = (share: number) => Math.round(share / 10);
	return (
		<div
			role="separator"
			tabIndex={0}
			className={styles.divider}
			data-layout={layout}
			aria-orientation={horizontal ? 'vertical' : 'horizontal'}
			aria-label={t('pair.divider.label')}
			aria-valuenow={percent(before)}
			aria-valuemin={percent(MIN_PANE_SHARE)}
			aria-valuemax={percent(before + after - MIN_PANE_SHARE)}
			aria-valuetext={tf('pair.divider.value', {
				first: percent(before),
				second: percent(after),
			})}
			onPointerDown={onPointerDown}
			onPointerMove={onPointerMove}
			onPointerUp={(event) => finish(event, true)}
			onPointerCancel={(event) => finish(event, false)}
			onDoubleClick={onReset}
			onKeyDown={onKeyDown}
		>
			<span className={styles.grip} aria-hidden="true" />
		</div>
	);
}
