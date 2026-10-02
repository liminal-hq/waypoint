// The system clipboard's files as the clipboard code uses it: set them, read them, hear that they changed
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Unsubscribe } from './vfsClient';

/** The files on the system clipboard: `file://` URIs, and whether they were cut (to be moved) or copied. */
export interface OsFiles {
	uris: string[];
	cut: boolean;
}

/**
 * Waypoint's side of the file clipboard other applications share (the `native-dnd` plugin's
 * `set_files` and `get_files`). `FakeOsClipboardClient` serves the same contract from memory.
 * Content that is not files (text, images) is never reported: `getFiles` is `null` for it.
 */
export interface OsClipboardClient {
	/** Whether the system file clipboard works here; where it does not, Waypoint's own clipboard is the only one. */
	available(): Promise<boolean>;
	/** Puts files on the system clipboard. Rejects where the compositor refuses it (Wayland wants a recent key press). */
	setFiles(files: OsFiles): Promise<void>;
	/** The files the system clipboard holds, or `null` when it holds none. */
	getFiles(): Promise<OsFiles | null>;
	/** Hears that the clipboard's owner changed, which may mean `getFiles` now answers differently. */
	onChange(listener: () => void): Unsubscribe;
}
