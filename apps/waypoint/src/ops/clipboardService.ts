// The window's clipboard: a mirror of Rust's shared clipboard, kept level with the system file clipboard
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createStore, type StoreApi } from 'zustand/vanilla';
import type {
	Clipboard,
	ClipboardMode,
	ListingHandle,
	Location,
	OpsClient,
	SelectionSpec,
} from '../services/opsClient';
import type { OsClipboardClient, OsFiles } from '../services/osClipboardClient';
import type { VfsClient } from '../services/vfsClient';
import { chooseClipboard, clipboardAsFiles } from './clipboardRules';

/** What a view reads: the clipboard as Rust last said it. */
export interface ClipboardState {
	clipboard: Clipboard;
}

export interface ClipboardService {
	store: StoreApi<ClipboardState>;
	/** Settles once Rust's clipboard has been read and the system clipboard looked at. */
	ready: Promise<void>;
	/** Cut or Copy of a selection of a listing this window opened. Resolves to what was set; rejects as the plugin does. */
	setFromSelection(
		handle: ListingHandle,
		spec: SelectionSpec,
		mode: ClipboardMode,
	): Promise<Clipboard>;
	/**
	 * The clipboard a paste uses, after the system clipboard has been looked at: files another
	 * application copied since are adopted first (see `chooseClipboard`).
	 */
	forPaste(): Promise<Clipboard>;
	/** Empties the clipboard, as pasting a cut does. */
	clear(): Promise<void>;
	/** Looks at the system clipboard and adopts files another application copied; false when there was nothing to adopt. */
	syncFromOs(): Promise<boolean>;
	/** Stops following Rust's clipboard, the system's and the window's focus. */
	dispose(): void;
}

export interface ClipboardServiceOptions {
	client: OpsClient;
	/** Turns the system clipboard's `file://` URIs into `Location`s (Rust parses them). */
	vfs: Pick<VfsClient, 'parseLocation'>;
	/** The system file clipboard; `null` where the window has none. */
	os: OsClipboardClient | null;
	/** The window whose focus makes the system clipboard worth looking at again; none in tests that drive `syncFromOs`. */
	focusTarget?: Pick<Window, 'addEventListener' | 'removeEventListener'> | null;
}

/** Any absolute `file://` URI parses against this; the base only matters for relative text. */
const ROOT: Location = { display: '/', uri: 'file:///' };

const EMPTY: Clipboard = { mode: 'copy', items: [], source: 'app', revision: 0 };

/**
 * Follows Rust's clipboard by revision, as the queue's store follows the queue: the change event
 * and a first read race, and whichever is newer wins. Every window runs one, so a Copy in one
 * window is the Paste of another: the clipboard is Rust's, shared, and this is a view of it.
 *
 * The system file clipboard is kept level in both directions. Copy and Cut also put their files
 * there (from the same handler as the key, since Wayland wants that). When the window gains focus,
 * and when the system says its clipboard changed, the files on it are looked at and, if another
 * application copied them since, adopted into Rust's clipboard (rule in `chooseClipboard`). Where
 * the system clipboard is unavailable, or refuses a write, Waypoint's own still works.
 */
export function createClipboardService(options: ClipboardServiceOptions): ClipboardService {
	const { client, vfs, os } = options;
	const store = createStore<ClipboardState>()(() => ({ clipboard: EMPTY }));
	let disposed = false;
	/** What the system clipboard held when this window last looked or wrote to it. */
	let lastSeen: OsFiles | null = null;
	let writing = 0;
	let usable: Promise<boolean> | null = null;

	const apply = (next: Clipboard): void => {
		if (disposed || next.revision <= store.getState().clipboard.revision) return;
		store.setState({ clipboard: next });
	};

	const available = (): Promise<boolean> => {
		if (!os) return Promise.resolve(false);
		usable ??= os.available().catch(() => false);
		return usable;
	};

	const stopEvents = client.onClipboard(apply);

	const publish = async (clipboard: Clipboard): Promise<void> => {
		const files = clipboardAsFiles(clipboard);
		if (!os || !files || !(await available())) return;
		writing += 1;
		try {
			await os.setFiles(files);
			lastSeen = files;
		} catch (error) {
			// Waypoint's own clipboard still works; only the other applications miss out.
			console.warn('could not put the files on the system clipboard', error);
		} finally {
			writing -= 1;
		}
	};

	const adopt = async (files: OsFiles): Promise<Clipboard | null> => {
		const items: Location[] = [];
		for (const uri of files.uris) {
			try {
				items.push(await vfs.parseLocation(uri, ROOT));
			} catch {
				// A URI Rust does not understand is not something Waypoint can paste.
			}
		}
		if (items.length === 0) return null;
		return client.setClipboard(files.cut ? 'cut' : 'copy', items, 'os');
	};

	const look = async (): Promise<Clipboard> => {
		const app = await client.getClipboard();
		apply(app);
		if (!os || writing > 0 || !(await available())) return app;
		let seen: OsFiles | null;
		try {
			seen = await os.getFiles();
		} catch {
			return app;
		}
		// Rust's clipboard moved on while the system's was being read (a Copy here): that is newer.
		if (writing > 0 || store.getState().clipboard.revision > app.revision) {
			return store.getState().clipboard;
		}
		const choice = chooseClipboard({ available: true, app, os: seen, lastSeen });
		lastSeen = seen;
		if (choice === 'app' || seen === null) return app;
		const adopted = await adopt(seen);
		if (!adopted) return app;
		apply(adopted);
		return adopted;
	};

	let looking: Promise<Clipboard> | null = null;
	/** One look at a time: a focus and a clipboard event arriving together read the system clipboard once. */
	const lookOnce = (): Promise<Clipboard> => {
		looking ??= look().finally(() => {
			looking = null;
		});
		return looking;
	};

	const onFocus = () => void lookOnce().catch(() => {});
	const stopOs = os?.onChange(onFocus);
	options.focusTarget?.addEventListener('focus', onFocus);

	const ready = Promise.all([
		client.getClipboard().then(apply, () => {}),
		lookOnce().then(
			() => {},
			() => {},
		),
	]).then(() => undefined);

	return {
		store,
		ready,
		async setFromSelection(handle, spec, mode) {
			const set = await client.setClipboardFromSelection(handle, spec, mode);
			apply(set);
			await publish(set);
			return set;
		},
		forPaste: lookOnce,
		async clear() {
			apply(await client.setClipboard('copy', []));
		},
		async syncFromOs() {
			const revision = store.getState().clipboard.revision;
			const after = await lookOnce();
			return after.revision > revision && after.source === 'os';
		},
		dispose() {
			disposed = true;
			stopEvents();
			stopOs?.();
			options.focusTarget?.removeEventListener('focus', onFocus);
		},
	};
}
