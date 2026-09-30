// THROWAWAY milestone 0 spike: typed wrappers over the spike commands
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Channel, invoke } from '@tauri-apps/api/core';

export interface Row {
	index: number;
	name: string;
	size: number;
	modified: number;
	kind: number;
}

export interface Created {
	handle: number;
	count: number;
	buildMs: number;
}

export interface ScanSummary {
	entries: number;
	batches: number;
	firstBatchMs: number;
	totalMs: number;
}

export const createSynthetic = (count: number) =>
	invoke<Created>('spike_create_synthetic', { count });

export const sortListing = (handle: number, by: string, desc = false) =>
	invoke<number>('spike_sort', { handle, by, desc });

export const rangeJson = (handle: number, start: number, count: number) =>
	invoke<Row[]>('spike_range', { handle, start, count });

const decoder = new TextDecoder();

/** Decodes the packed bytes from `spike_range_bin`. */
export function decodeRows(buffer: ArrayBuffer, startIndex: number): Row[] {
	const view = new DataView(buffer);
	const bytes = new Uint8Array(buffer);
	const rows: Row[] = [];
	let at = 0;
	let index = startIndex;
	while (at < buffer.byteLength) {
		const nameLength = view.getUint32(at, true);
		const size = Number(view.getBigUint64(at + 4, true));
		const modified = Number(view.getBigInt64(at + 12, true));
		const kind = view.getUint8(at + 20);
		const name = decoder.decode(bytes.subarray(at + 21, at + 21 + nameLength));
		rows.push({ index: index++, name, size, modified, kind });
		at += 21 + nameLength;
	}
	return rows;
}

export async function rangeBinary(handle: number, start: number, count: number): Promise<Row[]> {
	const buffer = await invoke<ArrayBuffer>('spike_range_bin', { handle, start, count });
	return decodeRows(buffer, start);
}

export const scan = (path: string, batch: number, onBatch: (rows: Row[]) => void) => {
	const channel = new Channel<{ rows: Row[] }>();
	channel.onmessage = (message) => onBatch(message.rows);
	return invoke<ScanSummary>('spike_scan', { path, batch, onBatch: channel });
};

export const scanBaseline = (path: string) => invoke<ScanSummary>('spike_scan_baseline', { path });
export const makeDir = (path: string, count: number) =>
	invoke<number>('spike_make_dir', { path, count });
export const watch = (path: string) => invoke<void>('spike_watch', { path });
export const watchEvents = () => invoke<number>('spike_watch_events');
export const unwatch = () => invoke<void>('spike_unwatch');
export const churn = (path: string, count: number) =>
	invoke<number>('spike_churn', { path, count });
export const removeDir = (path: string) => invoke<void>('spike_remove_dir', { path });
