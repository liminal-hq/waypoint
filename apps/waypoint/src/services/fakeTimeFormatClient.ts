// An in-memory TimeFormatClient for tests and the `?demo` window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { HourCycle, TimeFormatClient } from './timeFormatClient';
import type { Unsubscribe } from './vfsClient';

/** Holds an hour cycle that a test (or the demo) can change, announcing each change like the real client. */
export class FakeTimeFormatClient implements TimeFormatClient {
	private listeners = new Set<(hourCycle: HourCycle | undefined) => void>();
	/** How many times `get()` was called. */
	reads = 0;
	private failure = false;

	constructor(private hourCycle: HourCycle | undefined = undefined) {}

	/** Makes `get()` resolve `undefined`, as the real client does when the plugin errors. */
	failReads(): void {
		this.failure = true;
	}

	get(): Promise<HourCycle | undefined> {
		this.reads += 1;
		return Promise.resolve(this.failure ? undefined : this.hourCycle);
	}

	onChange(listener: (hourCycle: HourCycle | undefined) => void): Unsubscribe {
		this.listeners.add(listener);
		return () => {
			this.listeners.delete(listener);
		};
	}

	/** Changes the setting and tells every listener, as the plugin's change event does. */
	set(hourCycle: HourCycle | undefined): void {
		this.hourCycle = hourCycle;
		for (const listener of [...this.listeners]) listener(hourCycle);
	}
}
