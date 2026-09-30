// THROWAWAY milestone 0 spike: a virtualised list over a Rust-held listing, fetched by range
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useVirtualizer } from '@tanstack/react-virtual';
import { useCallback, useEffect, useRef, useState } from 'react';
import { rangeBinary, rangeJson, type Row } from './ipc';

export const ROW_HEIGHT = 28;
export const PAGE = 256;

export type FetchMode = 'json' | 'bin';

/** The live state the benchmark reads and drives. */
export interface ListHandle {
	container: HTMLDivElement | null;
	/** Rows currently rendered that have no data yet. */
	blankVisible(): number;
	visibleRange(): [number, number];
	invalidate(): void;
	selectAll(): void;
	selectedCount(): number;
	fetches: number[];
}

interface Props {
	handle: number;
	count: number;
	mode: FetchMode;
	expose(handle: ListHandle): void;
}

/** Selection kept as inclusive ranges, so selecting everything does not cost one entry per row. */
type Ranges = Array<[number, number]>;
const sizeOf = (ranges: Ranges) => ranges.reduce((n, [a, b]) => n + (b - a + 1), 0);
const has = (ranges: Ranges, i: number) => ranges.some(([a, b]) => i >= a && i <= b);

export function ListView({ handle, count, mode, expose }: Props) {
	const parent = useRef<HTMLDivElement | null>(null);
	const pages = useRef(new Map<number, Row[]>());
	const inflight = useRef(new Set<number>());
	const fetches = useRef<number[]>([]);
	const [, bump] = useState(0);
	const [generation, setGeneration] = useState(0);
	const [selection, setSelection] = useState<Ranges>([]);
	const anchor = useRef(0);

	const virtualizer = useVirtualizer({
		count,
		getScrollElement: () => parent.current,
		estimateSize: () => ROW_HEIGHT,
		overscan: 12,
	});
	const items = virtualizer.getVirtualItems();
	const first = items[0]?.index ?? 0;
	const last = items[items.length - 1]?.index ?? 0;

	const load = useCallback(
		(page: number) => {
			if (pages.current.has(page) || inflight.current.has(page)) return;
			inflight.current.add(page);
			const started = performance.now();
			const fetch = mode === 'bin' ? rangeBinary : rangeJson;
			void fetch(handle, page * PAGE, PAGE).then((rows) => {
				fetches.current.push(performance.now() - started);
				inflight.current.delete(page);
				pages.current.set(page, rows);
				bump((n) => n + 1);
			});
		},
		[handle, mode],
	);

	// Fetch what is visible plus a page either side.
	useEffect(() => {
		const from = Math.max(0, Math.floor(first / PAGE) - 1);
		const to = Math.min(Math.floor((count - 1) / PAGE), Math.floor(last / PAGE) + 1);
		for (let page = from; page <= to; page++) load(page);
	}, [first, last, count, load, generation]);

	const rowAt = (i: number) => pages.current.get(Math.floor(i / PAGE))?.[i % PAGE];

	useEffect(() => {
		expose({
			container: parent.current,
			blankVisible: () => {
				let blank = 0;
				const items = virtualizer.getVirtualItems();
				for (const item of items) if (!rowAt(item.index)) blank++;
				return blank;
			},
			visibleRange: () => {
				const live = virtualizer.getVirtualItems();
				return [live[0]?.index ?? 0, live[live.length - 1]?.index ?? 0];
			},
			invalidate: () => {
				pages.current.clear();
				inflight.current.clear();
				setGeneration((g) => g + 1);
			},
			selectAll: () => setSelection([[0, count - 1]]),
			selectedCount: () => sizeOf(selection),
			fetches: fetches.current,
		});
	});

	const onClick = (i: number, shift: boolean) => {
		if (shift) setSelection([[Math.min(anchor.current, i), Math.max(anchor.current, i)]]);
		else {
			anchor.current = i;
			setSelection([[i, i]]);
		}
	};

	return (
		<div
			ref={parent}
			data-spike-list=""
			tabIndex={0}
			onKeyDown={(event) => {
				if ((event.ctrlKey || event.metaKey) && event.key === 'a') {
					event.preventDefault();
					setSelection([[0, count - 1]]);
				}
			}}
			style={{ height: '100%', overflow: 'auto', contain: 'strict' }}
		>
			<div style={{ height: virtualizer.getTotalSize(), width: '100%', position: 'relative' }}>
				{items.map((item) => {
					const row = rowAt(item.index);
					const selected = has(selection, item.index);
					return (
						<div
							key={item.key}
							data-row={item.index}
							onClick={(event) => onClick(item.index, event.shiftKey)}
							style={{
								position: 'absolute',
								top: 0,
								left: 0,
								width: '100%',
								height: ROW_HEIGHT,
								transform: `translateY(${item.start}px)`,
								display: 'flex',
								gap: 12,
								alignItems: 'center',
								padding: '0 12px',
								boxSizing: 'border-box',
								background: selected ? 'rgba(120,160,255,0.25)' : 'transparent',
								color: row ? 'inherit' : 'rgba(255,255,255,0.25)',
								fontSize: 13,
							}}
						>
							<span style={{ width: 72, opacity: 0.6 }}>{item.index}</span>
							<span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
								{row ? row.name : '…'}
							</span>
							<span style={{ width: 90, textAlign: 'right' }}>{row ? row.size : ''}</span>
							<span style={{ width: 110, textAlign: 'right' }}>{row ? row.modified : ''}</span>
						</div>
					);
				})}
			</div>
		</div>
	);
}
