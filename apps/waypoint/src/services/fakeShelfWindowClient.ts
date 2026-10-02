// An in-memory ShelfWindowClient for tests: it records the calls and lets a test change what is shown
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ShelfWindowClient } from './shelfWindowClient';

export class FakeShelfWindowClient implements ShelfWindowClient {
	shown = true;
	/** Whether a Shelf window exists to be raised. */
	exists = true;
	readonly calls: string[] = [];
	private readonly listeners = new Set<(shown: boolean) => void>();

	async toggle(): Promise<boolean> {
		this.calls.push('toggle');
		if (!this.exists) return false;
		this.show(!this.shown);
		return this.shown;
	}

	async raise(): Promise<boolean> {
		this.calls.push('raise');
		if (!this.exists) return false;
		this.show(true);
		return true;
	}

	async visible(): Promise<boolean> {
		this.calls.push('visible');
		return this.exists && this.shown;
	}

	onVisibleChange(listener: (shown: boolean) => void): () => void {
		this.listeners.add(listener);
		return () => {
			this.listeners.delete(listener);
		};
	}

	/** Shows or hides the window, and tells the listeners, as Rust's event does. */
	show(shown: boolean): void {
		this.shown = shown;
		for (const listener of [...this.listeners]) listener(shown);
	}
}
