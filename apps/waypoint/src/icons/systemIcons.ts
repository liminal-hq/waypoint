// The System icon set's shared state: whether the OS can supply file and folder icons, the OS look they depend on, and which addresses have loaded
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useCallback, useSyncExternalStore } from 'react';
import type { TypeIconTarget } from '@liminal-hq/plugin-mime-apps';
import type {
	SystemIconAvailability,
	SystemIconsClient,
	SystemLook,
} from '../services/systemIconsClient';
import { createTauriSystemIconsClient } from '../services/tauriSystemIconsClient';
import type { Unsubscribe } from '../services/vfsClient';
import type { FolderTone } from './portage/portagePalette';

/** What the window knows about the system's icons. */
export interface SystemIconsState {
	/** `loading` until the plugin has answered once; the Waypoint icons are drawn until then. */
	phase: 'loading' | 'ready';
	/** Icons for types of file. */
	type: SystemIconAvailability;
	/** Icons for folders. */
	folder: SystemIconAvailability;
	/** The icon theme the OS reports, part of every address so a change of theme is a new picture. */
	theme: string | null;
	/** Raised whenever the OS look changes, so every address is new and every icon is asked for again. */
	revision: number;
}

const NOT_YET: SystemIconAvailability = { available: false, reason: null };

const INITIAL: SystemIconsState = {
	phase: 'loading',
	type: NOT_YET,
	folder: NOT_YET,
	theme: null,
	revision: 0,
};

/** How many addresses are remembered before the lot are forgotten and asked for again as rows need them. */
const MAX_IMAGES = 2048;

type ImageState = 'loading' | 'loaded' | 'missing';

let client: SystemIconsClient | null = null;
let state: SystemIconsState = INITIAL;
let lastLook: SystemLook | null = null;
let stopLook: Unsubscribe | null = null;
let starting = false;
let epoch = 0;
const listeners = new Set<() => void>();
const images = new Map<string, ImageState>();
const probes = new Map<string, Unsubscribe>();
const imageListeners = new Map<string, Set<() => void>>();

function theClient(): SystemIconsClient {
	client ??= createTauriSystemIconsClient();
	return client;
}

function set(next: SystemIconsState): void {
	state = next;
	for (const listener of [...listeners]) listener();
}

function forgetImages(): void {
	for (const stop of probes.values()) stop();
	probes.clear();
	images.clear();
}

/** The sentence for a plugin that did not answer. */
function noAnswer(error: unknown): SystemIconAvailability {
	return { available: false, reason: error instanceof Error ? error.message : String(error) };
}

async function load(mine: number): Promise<void> {
	const source = theClient();
	const [status, look] = await Promise.allSettled([source.status(), source.look()]);
	if (mine !== epoch) return;
	lastLook = look.status === 'fulfilled' ? look.value : null;
	set({
		phase: 'ready',
		type: status.status === 'fulfilled' ? status.value.typeIcons : noAnswer(status.reason),
		folder: status.status === 'fulfilled' ? status.value.folderIcons : noAnswer(status.reason),
		theme: lastLook?.theme ?? null,
		revision: state.revision,
	});
	if (listeners.size > 0 && !stopLook)
		stopLook = source.onLookChanged((next) => changed(mine, next));
}

/** The OS changed its icon theme or its colour scheme: forget what was made, ask the plugin to as well, and repaint with new addresses. */
function changed(mine: number, next: SystemLook): void {
	if (mine !== epoch) return;
	if (lastLook && lastLook.theme === next.theme && lastLook.scheme === next.scheme) return;
	lastLook = next;
	const source = theClient();
	void (async () => {
		await source.refresh().catch(() => undefined);
		// The theme may be one whose icons were not there before: ask again what can be drawn.
		const status = await source.status().catch(() => null);
		if (mine !== epoch) return;
		forgetImages();
		set({
			...state,
			...(status ? { type: status.typeIcons, folder: status.folderIcons } : {}),
			theme: next.theme,
			revision: state.revision + 1,
		});
	})();
}

function subscribe(listener: () => void): Unsubscribe {
	listeners.add(listener);
	if (listeners.size === 1) {
		if (state.phase === 'ready') {
			stopLook ??= theClient().onLookChanged((next) => changed(epoch, next));
		} else if (!starting) {
			starting = true;
			void load(epoch).finally(() => {
				starting = false;
			});
		}
	}
	return () => {
		listeners.delete(listener);
		if (listeners.size === 0) {
			stopLook?.();
			stopLook = null;
		}
	};
}

/**
 * Whether the OS can supply file and folder icons, and the look they were last drawn for. Reading it
 * starts the plugin's status check and the listening for OS changes, once for every icon on the page.
 */
export function useSystemIcons(): SystemIconsState {
	return useSyncExternalStore(
		subscribe,
		() => state,
		() => state,
	);
}

/** Whether the Settings page may offer the System icon set, and why not when it may not. */
export interface SystemOffer {
	/** The plugin has not answered yet; the page does not say the set is unavailable before it has. */
	loading: boolean;
	/** Some kind of icon can be had from the system. */
	offered: boolean;
	/** The sentence that says why not, when it is not offered and the system said. */
	reason: string | null;
}

/** The System set is offered when the system can supply icons for types or for folders (the rest fall back to the Waypoint glyph one by one). */
export function useSystemOffer(): SystemOffer {
	const { phase, type, folder } = useSystemIcons();
	return {
		loading: phase === 'loading',
		offered: type.available || folder.available,
		reason: type.reason ?? folder.reason,
	};
}

/** The address of the system's icon for `target`, in the look the state describes. `tone` changes only the address, so an icon asked for in the other tone is a new request that the plugin answers from its cache. */
export function systemIconUrl(
	target: TypeIconTarget,
	options: { size: number; scale: number; tone: FolderTone },
	look: Pick<SystemIconsState, 'theme' | 'revision'> = state,
): string {
	const url = theClient().url(target, {
		size: options.size,
		scale: options.scale,
		theme: look.theme,
		revision: look.revision,
	});
	return `${url}&tone=${options.tone}`;
}

/** The address of the icon stored in an entry's file (a program's, a shortcut's), named by its listing token. It has no theme or tone: the shell draws it from the file. */
export function systemFileIconUrl(
	token: string,
	options: { size: number; scale: number; modifiedMs: number | null },
): string {
	return theClient().fileUrl(token, options);
}

function notifyImage(url: string): void {
	for (const listener of [...(imageListeners.get(url) ?? [])]) listener();
}

/** Starts loading an address once; every row showing the same type shares the one load. */
export function requestSystemImage(url: string): void {
	if (images.has(url)) return;
	if (images.size >= MAX_IMAGES) forgetImages();
	images.set(url, 'loading');
	notifyImage(url);
	probes.set(
		url,
		theClient().probe(url, (loaded) => {
			probes.delete(url);
			images.set(url, loaded ? 'loaded' : 'missing');
			notifyImage(url);
		}),
	);
}

/**
 * Where an address stands: `idle` before anyone asked, then `loading`, then `loaded` or `missing` (the
 * system has no icon for it). One load serves every icon with the same address.
 */
export function useSystemImage(url: string | null): ImageState | 'idle' {
	const subscribeImage = useCallback(
		(listener: () => void): Unsubscribe => {
			if (url === null) return () => undefined;
			let bucket = imageListeners.get(url);
			if (!bucket) imageListeners.set(url, (bucket = new Set()));
			bucket.add(listener);
			return () => {
				bucket.delete(listener);
				if (bucket.size === 0) imageListeners.delete(url);
			};
		},
		[url],
	);
	return useSyncExternalStore(
		subscribeImage,
		() => (url === null ? 'idle' : (images.get(url) ?? 'idle')),
		() => 'idle',
	);
}

/** How many addresses are being remembered, for the tests that check a listing needs few. */
export function systemImageCount(): number {
	return images.size;
}

/** Puts the store back as it started and swaps the client, for tests; `null` goes back to the real one. */
export function configureSystemIcons(next: SystemIconsClient | null): void {
	epoch += 1;
	stopLook?.();
	stopLook = null;
	starting = false;
	forgetImages();
	imageListeners.clear();
	lastLook = null;
	client = next;
	state = INITIAL;
	for (const listener of [...listeners]) listener();
	// Mounted icons start the new client.
	if (listeners.size > 0) {
		starting = true;
		void load(epoch).finally(() => {
			starting = false;
		});
	}
}
