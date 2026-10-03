// An in-memory PropertiesWindowClient that keeps the factory's rules: four at most, the same subject reuses its window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import {
	MAX_PROPERTIES_WINDOWS,
	subjectKey,
	type OpenOutcome,
	type PropertiesWindowClient,
} from './propertiesWindowClient';

/**
 * Serves the contract from memory. `windows` is what is open (label to subject) and `focused`
 * the labels brought to the front, in order. `close` frees a window's place. A fake made with a
 * `subject` is itself one Properties window, which is what the screen under test asks.
 */
export class FakePropertiesWindowClient implements PropertiesWindowClient {
	readonly windows = new Map<string, Location>();
	readonly focused: string[] = [];
	/** What `subject` answers, and what `setSubject` last recorded. */
	current: Location | null;
	private next = 0;

	constructor(subject: Location | null = null) {
		this.current = subject;
	}

	async open(location: Location): Promise<OpenOutcome> {
		const key = subjectKey(location.uri);
		for (const [label, subject] of this.windows) {
			if (subjectKey(subject.uri) === key) {
				this.focused.push(label);
				return 'focused';
			}
		}
		if (this.windows.size >= MAX_PROPERTIES_WINDOWS) return 'limit';
		this.windows.set(`properties-${++this.next}`, location);
		return 'opened';
	}

	close(label: string): void {
		this.windows.delete(label);
	}

	async subject(): Promise<Location> {
		if (!this.current) throw new Error('not a Properties window');
		return this.current;
	}

	async setSubject(location: Location): Promise<void> {
		this.current = location;
	}
}
