// Supplies a window's operations queue (its client and mirrored store) to the ring, the popover and the panel
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from 'react';
import { useStore } from 'zustand';
import { createStore } from 'zustand/vanilla';
import type { Location, OpsClient } from '../services/opsClient';
import type { OsClipboardClient } from '../services/osClipboardClient';
import { jobViews, type JobView } from './jobText';
import { withProgress, type JobWithProgress } from './opsSelectors';
import { createOpsStore, type OpsHandle, type OpsState, type ProgressState } from './opsStore';

export interface OpsContextValue {
	handle: OpsHandle;
	/** The window's webview label; jobs it started carry it as their `originWindow`. */
	windowLabel: string;
	/** Opens the Operations window; `null` where there is nothing to pop out (that window itself). */
	popOut: (() => void) | null;
	/** Shows a finished job's destination in this window; `null` where the window cannot. */
	showInFolder: ((location: Location) => void) | null;
	/** The system file clipboard Cut and Copy keep level with; `null` where the window has none. */
	osClipboard: OsClipboardClient | null;
}

const OpsContext = createContext<OpsContextValue | null>(null);

interface OpsProviderProps {
	client: OpsClient;
	windowLabel: string;
	popOut?: (() => void) | null;
	showInFolder?: ((location: Location) => void) | null;
	osClipboard?: OsClipboardClient | null;
	/** Receives the store once it exists, for what lives outside React (the notices, the announcer). */
	onHandle?: (handle: OpsHandle) => void | (() => void);
	children: ReactNode;
}

/**
 * Follows `client`'s queue for as long as it is mounted. Children render at once: the ring is always
 * in the status bar and shows an idle state until the first snapshot arrives.
 */
export function OpsProvider({
	client,
	windowLabel,
	popOut = null,
	showInFolder = null,
	osClipboard = null,
	onHandle,
	children,
}: OpsProviderProps) {
	const [handle, setHandle] = useState<OpsHandle | null>(null);
	useEffect(() => {
		const created = createOpsStore(client);
		setHandle(created);
		const cleanup = onHandle?.(created);
		return () => {
			cleanup?.();
			created.dispose();
		};
	}, [client, onHandle]);
	const value = useMemo(
		() =>
			handle?.client === client ? { handle, windowLabel, popOut, showInFolder, osClipboard } : null,
		[client, handle, windowLabel, popOut, showInFolder, osClipboard],
	);
	return <OpsContext.Provider value={value}>{children}</OpsContext.Provider>;
}

/** The queue for this window, or `null` outside a provider and before the store exists. */
export function useOps(): OpsContextValue | null {
	return useContext(OpsContext);
}

const NO_JOBS: JobWithProgress[] = [];

/** Every job in queue order with its freshest progress; empty before the first snapshot. */
export function useOpsJobs(): JobWithProgress[] {
	const ops = useOps();
	const fallback = useMemo(() => createEmptyStores(), []);
	const state = useStore(ops?.handle.store ?? fallback.store, (s) => s);
	const ticks = useStore(ops?.handle.progress ?? fallback.progress, (s) => s.ticks);
	return useMemo(
		() =>
			state.snapshot
				? state.snapshot.jobs.map((job) =>
						withProgress(job, state.jobRevisions[job.id] ?? 0, ticks[job.id]),
					)
				: NO_JOBS,
		[state, ticks],
	);
}

/** Whether Pause all is in force; `false` before the first snapshot. */
export function useOpsPaused(): boolean {
	const ops = useOps();
	const fallback = useMemo(() => createEmptyStores(), []);
	return useStore(ops?.handle.store ?? fallback.store, (s) => s.snapshot?.paused ?? false);
}

/** The rows for the queue UI. */
export function useOpsViews(): JobView[] {
	const ops = useOps();
	const jobs = useOpsJobs();
	const canShow = ops?.showInFolder != null;
	return useMemo(() => jobViews(jobs, canShow), [jobs, canShow]);
}

// Hooks cannot be skipped, so a window with no provider reads a pair of empty stores.
function createEmptyStores() {
	return {
		store: createStore<OpsState>()(() => ({ snapshot: null, jobRevisions: {}, resyncs: 0 })),
		progress: createStore<ProgressState>()(() => ({ ticks: {} })),
	};
}
