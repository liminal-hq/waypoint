// An in-memory OsClipboardClient, with a method that stands in for another application copying files
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { OsClipboardClient, OsFiles } from './osClipboardClient';

export interface FakeOsClipboardOptions {
	/** `false` makes the feature unavailable, as on a system the plugin cannot reach. */
	available?: boolean;
	/** `true` makes `setFiles` reject, as Wayland does without a recent key press. */
	refuseSet?: boolean;
}

/** The system clipboard in memory. Tests drive it with `external` and read what Waypoint put there with `files`. */
export class FakeOsClipboardClient implements OsClipboardClient {
	private held: OsFiles | null = null;
	private readonly listeners = new Set<() => void>();
	private readonly enabled: boolean;
	refuseSet: boolean;
	/** Every call, in order: `['setFiles', files]`, `['getFiles']`. */
	readonly calls: unknown[][] = [];

	constructor(options: FakeOsClipboardOptions = {}) {
		this.enabled = options.available ?? true;
		this.refuseSet = options.refuseSet ?? false;
	}

	async available(): Promise<boolean> {
		return this.enabled;
	}

	async setFiles(files: OsFiles): Promise<void> {
		this.calls.push(['setFiles', structuredClone(files)]);
		if (!this.enabled) throw { kind: 'unsupported', message: 'no file clipboard' };
		if (this.refuseSet) throw { kind: 'io', message: 'the compositor refused the selection' };
		this.held = structuredClone(files);
		this.notify();
	}

	async getFiles(): Promise<OsFiles | null> {
		this.calls.push(['getFiles']);
		if (!this.enabled) throw { kind: 'unsupported', message: 'no file clipboard' };
		return this.held ? structuredClone(this.held) : null;
	}

	onChange(listener: () => void): () => void {
		this.listeners.add(listener);
		return () => {
			this.listeners.delete(listener);
		};
	}

	/** What the system clipboard holds now. */
	get files(): OsFiles | null {
		return this.held ? structuredClone(this.held) : null;
	}

	/** Another application copies (or cuts) files, or puts something that is not files (`null`). */
	external(files: OsFiles | null): void {
		this.held = files ? structuredClone(files) : null;
		this.notify();
	}

	private notify(): void {
		for (const listener of [...this.listeners]) listener();
	}
}
