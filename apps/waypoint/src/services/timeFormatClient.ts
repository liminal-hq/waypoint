// The frontend's view of the system's 12/24-hour clock setting
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Unsubscribe } from './vfsClient';

/** What `Intl.DateTimeFormat` takes as `hourCycle` for a 24-hour or a 12-hour clock. */
export type HourCycle = 'h23' | 'h12';

/**
 * Where the 12/24-hour preference comes from. A reading is `undefined` when the system's setting
 * is unknown (the plugin is unavailable, errored, or could only guess), which leaves the hour
 * cycle to `Intl` and the locale, as it was before the setting was read.
 */
export interface TimeFormatClient {
	/** The user's hour cycle now. Never rejects: a failure resolves `undefined`. */
	get(): Promise<HourCycle | undefined>;
	/** Hears the new hour cycle whenever the user changes the setting while the app runs. */
	onChange(listener: (hourCycle: HourCycle | undefined) => void): Unsubscribe;
}
