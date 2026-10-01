// The real TimeFormatClient: reads the system clock setting through the os-prefs plugin's guest-js API
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { getTimeFormat, hourCycleOf, onTimeFormatChanged } from '@liminal-hq/plugin-os-prefs';
import type { HourCycle, TimeFormatClient } from './timeFormatClient';

/** A `TimeFormatClient` over the `os-prefs` plugin. Create one per window. */
export function createTauriTimeFormatClient(): TimeFormatClient {
	let warned = false;
	const warnOnce = (error: unknown) => {
		if (warned) return;
		warned = true;
		console.warn('The system time format is unavailable; the locale decides it.', error);
	};
	return {
		async get() {
			try {
				return hourCycleOf(await getTimeFormat());
			} catch (error) {
				warnOnce(error);
				return undefined;
			}
		},
		onChange(listener) {
			let stopped = false;
			let unlisten: (() => void) | null = null;
			onTimeFormatChanged((format) => listener(hourCycleOf(format))).then((stop) => {
				if (stopped) stop();
				else unlisten = stop;
			}, warnOnce);
			return () => {
				stopped = true;
				unlisten?.();
				unlisten = null;
			};
		},
	};
}

export type { HourCycle };
