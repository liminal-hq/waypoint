// THROWAWAY milestone 0 spike: hosts the list and exposes the benchmark on window.__spike
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useRef, useState } from 'react';
import * as bench from './bench';
import { runAll } from './auto';
import * as ipc from './ipc';
import { ListView, type FetchMode, type ListHandle } from './ListView';

interface Loaded {
	handle: number;
	count: number;
	mode: FetchMode;
}

declare global {
	interface Window {
		__spike: unknown;
		__spikeLoad: (count: number, mode: FetchMode) => Promise<ipc.Created>;
	}
}

export function SpikeScreen() {
	const [loaded, setLoaded] = useState<Loaded | null>(null);
	const list = useRef<ListHandle | null>(null);
	const current = useRef<Loaded | null>(null);

	useEffect(() => {
		const load = async (count: number, mode: FetchMode) => {
			const created = await ipc.createSynthetic(count);
			const next = { handle: created.handle, count, mode };
			current.current = next;
			setLoaded(next);
			await new Promise((resolve) => setTimeout(resolve, 400));
			return created;
		};
		window.__spikeLoad = load;
		window.__spike = {
			ipc,
			bench,
			load,
			list: () => list.current,
			state: () => current.current,
			sweep: (px: number, frames: number, from = 0) => bench.sweep(list.current!, px, frames, from),
			jumps: (times: number) => bench.jumps(list.current!, current.current!.count, times),
			selection: () => bench.selection(list.current!, current.current!.count),
			sorts: () => bench.sorts(list.current!, current.current!.handle),
			ipcBench: () => bench.ipc(current.current!.handle, current.current!.count),
		};
		if (location.hash.startsWith('#spike-auto')) {
			void runAll({
				load: (count, mode) => window.__spikeLoad(count, mode),
				list: () => list.current,
				handle: () => current.current!.handle,
			});
		}
	}, []);

	return (
		<div
			style={{
				position: 'fixed',
				inset: 0,
				display: 'flex',
				flexDirection: 'column',
				background: 'var(--wp-bg, #1b1916)',
				color: 'var(--wp-fg, #eee)',
				fontFamily: 'system-ui, sans-serif',
			}}
		>
			<div style={{ padding: '6px 12px', fontSize: 12, opacity: 0.7 }}>
				spike — {loaded ? `${loaded.count.toLocaleString()} rows, ${loaded.mode}` : 'not loaded'}
			</div>
			<div style={{ flex: 1, minHeight: 0 }}>
				{loaded ? (
					<ListView
						key={`${loaded.handle}-${loaded.mode}`}
						handle={loaded.handle}
						count={loaded.count}
						mode={loaded.mode}
						expose={(handle) => (list.current = handle)}
					/>
				) : null}
			</div>
		</div>
	);
}
