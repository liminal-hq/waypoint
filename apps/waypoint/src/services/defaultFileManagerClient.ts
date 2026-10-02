// Making Waypoint the default file manager: what the system allows, who is default now, and doing it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/** The type every folder has, which a "default file manager" is the default application for. */
export const DIRECTORY_TYPE = 'inode/directory';

/**
 * Waypoint's desktop file, named by the bundle identifier. The file ships with the installed
 * package; a development run has none, and `make` then says so.
 */
export const WAYPOINT_DESKTOP_ID = 'ca.liminalhq.waypoint.desktop';

/** How the system lets Waypoint become the default for folders. */
export type FileManagerAction =
	/** The default is changed here (Linux). */
	| { kind: 'set' }
	/** Only the person can change it, in the system's Default apps page, which `make` opens (Windows). */
	| { kind: 'settings' }
	/** Not offered, with the sentence for why (a Flatpak sandbox, an unsupported system). */
	| { kind: 'unavailable'; reason: string };

/** Who opens folders now. */
export interface CurrentFileManager {
	isWaypoint: boolean;
	/** The name of the application that does, when it is not Waypoint and there is one. */
	name: string | null;
}

export interface DefaultFileManagerClient {
	action(): Promise<FileManagerAction>;
	/** Who is the default for folders now; `null` where that cannot be read. */
	current(): Promise<CurrentFileManager | null>;
	/**
	 * Makes Waypoint the default (`set`), or opens the system's Default apps page (`settings`).
	 * Rejects with a sentence for people when it cannot (no desktop file, the system refused).
	 */
	make(): Promise<void>;
}
