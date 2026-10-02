// What Overview knows about the size of the home folder, read from the directory-size scan (A70)
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { useCallback, useEffect, useMemo, useSyncExternalStore } from 'react';
import { useSettings, useSettingsReady } from '../settings/SettingsContext';
import { useHomeScanStore } from './HomeScanContext';
import type { HomeScanState } from './homeScanStore';

/** The size of the home folder, as far as it has been measured. */
export type HomeMeasure =
	| { status: 'notMeasured' }
	| { status: 'measuring' }
	| {
			status: 'done';
			/** The bytes the home folder holds. */
			bytes: number;
			/** When the measurement finished, in milliseconds since the Unix epoch; `null` when it is unknown. */
			asOfMs: number | null;
			/** A fresh scan is running; `bytes` is the last complete result's. */
			refreshing?: boolean;
	  };

export const NOT_MEASURED: HomeMeasure = { status: 'notMeasured' };

/** A cached result younger than this is shown without measuring again. */
export const FRESH_MS = 60 * 60 * 1000;

const EMPTY: HomeScanState = {
	root: null,
	running: false,
	shown: null,
	final: null,
	current: null,
	outcome: 'none',
	error: null,
};

/** The Home stat's reading of a scan: the last complete result (even while a fresh scan runs), else "measuring" or "not measured". */
export function measureOf(scan: HomeScanState): HomeMeasure {
	if (scan.final) {
		return {
			status: 'done',
			bytes: scan.final.totalBytes,
			asOfMs: scan.final.measuredAtMs > 0 ? scan.final.measuredAtMs : null,
			...(scan.running ? { refreshing: true } : {}),
		};
	}
	return scan.running ? { status: 'measuring' } : NOT_MEASURED;
}

export interface HomeUsage {
	/** Whether the window has a directory-size scan at all. */
	available: boolean;
	scan: HomeScanState;
	measure: HomeMeasure;
	/** Starts a measurement (does nothing while one runs). */
	measureNow(): void;
	cancel(): void;
}

/**
 * Overview's measurement of `home`. While Overview is mounted it reads the cached result at once
 * and, when "Measure Home when Overview opens" is on and the cache is missing or over an hour old,
 * starts a scan. Leaving Overview (the tab closes, or it moves to another location) cancels a
 * running scan. The setting is waited for, so a person who turned it off never sees a scan begin.
 */
export function useHomeMeasure(home: Location | null): HomeUsage {
	const store = useHomeScanStore();
	const auto = useSettings((settings) => settings.previews.measureHomeOnOpen);
	const settingsReady = useSettingsReady();
	const subscribe = store?.subscribe ?? (() => () => {});
	const snapshot = store?.getSnapshot ?? (() => EMPTY);
	const scan = useSyncExternalStore(subscribe, snapshot);
	const uri = home?.uri;

	useEffect(() => {
		if (!store || !home || !settingsReady) return;
		let live = true;
		void store.load(home).then((cached) => {
			if (!live || !auto) return;
			const fresh = cached !== null && Date.now() - cached.measuredAtMs < FRESH_MS;
			if (!fresh) store.start(home);
		});
		return () => {
			live = false;
			store.cancel();
		};
		// `home` is read through its address: a new object for the same folder must not restart the scan.
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [store, uri, settingsReady]);

	const measureNow = useCallback(() => {
		if (store && home) store.start(home);
	}, [store, home]);
	const cancel = useCallback(() => store?.cancel(), [store]);
	const measure = useMemo(() => measureOf(scan), [scan]);
	return { available: store !== null, scan, measure, measureNow, cancel };
}
