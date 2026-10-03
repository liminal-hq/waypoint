// The frontend's view of the Properties windows: asking for one, and a window asking what it is about
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';

/** What asking for a Properties window did. */
export type OpenOutcome = 'opened' | 'focused' | 'limit';

/** The most Properties windows open at once; the fifth request is refused (D122). */
export const MAX_PROPERTIES_WINDOWS = 4;

/**
 * Names the subject of a window so the same folder or file is recognised however its location
 * was written (a trailing slash does not make a different subject). Mirrors `subject_key` in
 * `src-tauri/src/properties_window.rs`.
 */
export function subjectKey(uri: string): string {
	const trimmed = uri.replace(/\/+$/, '');
	return trimmed === '' || trimmed.endsWith(':') ? uri : trimmed;
}

/**
 * The Properties windows, made by a factory in `src-tauri` (A69). A main window asks for one with
 * `open`; the window itself asks what it is about with `subject` and tells Rust when it follows
 * its entry to a new name with `setSubject`. `TauriPropertiesWindowClient` calls the app's
 * commands; `FakePropertiesWindowClient` keeps the same rules in memory.
 */
export interface PropertiesWindowClient {
	/** Opens a window for `location`, or brings the one that has it to the front; `limit` when four are open. */
	open(location: Location): Promise<OpenOutcome>;
	/** What the calling window is about. Rejects in a window that is not a Properties window. */
	subject(): Promise<Location>;
	/** Records that the calling window now follows `location`. */
	setSubject(location: Location): Promise<void>;
}
