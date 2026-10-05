// An in-memory SystemIconsClient: answers what the test sets, records what was asked for, and loads an address when the test says so
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { TypeIconOptions, TypeIconTarget } from '@liminal-hq/plugin-mime-apps';
import type {
	SystemIconAvailability,
	SystemIconsClient,
	SystemIconsStatus,
	SystemLook,
} from './systemIconsClient';

const AVAILABLE: SystemIconAvailability = { available: true, reason: null };

export interface FakeSystemIcons extends SystemIconsClient {
	/** The addresses asked to be loaded, in order, one per `probe` call. */
	readonly probed: string[];
	/** How many times `refresh` ran. */
	readonly refreshed: number;
	/** Settles every address waiting: a picture came back (`true`) or a 404 (`false`). */
	settle(loaded: boolean): void;
	/** Settles one address. */
	settleUrl(url: string, loaded: boolean): void;
	/** Changes the OS look as the desktop would, and announces it. */
	changeLook(next: Partial<SystemLook>): void;
	/** Makes `status` reject, as a plugin that is not there would. */
	failStatus(): void;
	readonly listenerCount: number;
}

export function createFakeSystemIconsClient(
	options: {
		status?: Partial<SystemIconsStatus>;
		look?: Partial<SystemLook>;
	} = {},
): FakeSystemIcons {
	const status: SystemIconsStatus = {
		typeIcons: AVAILABLE,
		folderIcons: AVAILABLE,
		...options.status,
	};
	let current: SystemLook = { theme: 'Adwaita', scheme: 'light', ...options.look };
	let failing = false;
	let refreshed = 0;
	const probed: string[] = [];
	const waiting = new Map<string, Set<(loaded: boolean) => void>>();
	const listeners = new Set<(look: SystemLook) => void>();
	return {
		status: () => (failing ? Promise.reject(new Error('no plugin')) : Promise.resolve(status)),
		look: () => Promise.resolve(current),
		onLookChanged(listener) {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
		refresh: () => {
			refreshed += 1;
			return Promise.resolve();
		},
		url(target: TypeIconTarget, urlOptions: TypeIconOptions) {
			const [kind, value] =
				'mime' in target
					? ['mime', target.mime]
					: 'extension' in target
						? ['ext', target.extension]
						: ['folder', target.folder];
			const params = [`size=${urlOptions.size ?? 32}`, `scale=${urlOptions.scale ?? 1}`];
			if (urlOptions.theme) params.push(`theme=${urlOptions.theme}`);
			if (urlOptions.revision) params.push(`v=${urlOptions.revision}`);
			return `fake://${kind}/${value}?${params.join('&')}`;
		},
		fileUrl: (token, fileOptions) =>
			`fake://file/${token}?size=${fileOptions.size}&scale=${fileOptions.scale}&m=${fileOptions.modifiedMs ?? 0}`,
		probe(url, done) {
			probed.push(url);
			let callbacks = waiting.get(url);
			if (!callbacks) waiting.set(url, (callbacks = new Set()));
			callbacks.add(done);
			return () => callbacks.delete(done);
		},
		probed,
		get refreshed() {
			return refreshed;
		},
		settle(loaded) {
			for (const url of [...waiting.keys()]) this.settleUrl(url, loaded);
		},
		settleUrl(url, loaded) {
			const callbacks = waiting.get(url);
			waiting.delete(url);
			for (const callback of callbacks ?? []) callback(loaded);
		},
		changeLook(next) {
			current = { ...current, ...next };
			for (const listener of [...listeners]) listener(current);
		},
		failStatus() {
			failing = true;
		},
		get listenerCount() {
			return listeners.size;
		},
	};
}
