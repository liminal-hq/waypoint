// The system's own file and folder icons, as the page uses them: whether they can be had, what the OS look is, and the address of one
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { TypeIconOptions, TypeIconTarget } from '@liminal-hq/plugin-mime-apps';
import type { Unsubscribe } from './vfsClient';

/** Whether one kind of system icon can be drawn here, and the sentence that says why not when it cannot. */
export interface SystemIconAvailability {
	available: boolean;
	reason: string | null;
}

export interface SystemIconsStatus {
	/** Icons for types of file. */
	typeIcons: SystemIconAvailability;
	/** Icons for folders, standard ones included. */
	folderIcons: SystemIconAvailability;
}

/** The parts of the OS look that change what its icons are: the icon theme and the colour scheme. */
export interface SystemLook {
	/** The icon theme the OS reports, when it reports one. */
	theme: string | null;
	scheme: string;
}

export interface SystemIconsClient {
	/** What can be drawn here; rejects where the plugin is not there. */
	status(): Promise<SystemIconsStatus>;
	/** The OS look now; rejects where it cannot be read. */
	look(): Promise<SystemLook>;
	/** Calls `listener` whenever the OS look changes. */
	onLookChanged(listener: (look: SystemLook) => void): Unsubscribe;
	/** Empties the icons made so far, so the next ones are drawn from the theme now in force. */
	refresh(): Promise<void>;
	/** The address of the icon for a target. */
	url(target: TypeIconTarget, options: TypeIconOptions): string;
	/** Loads an address once and says whether a picture came back: a 404 is `false`. Returns a cancel. */
	probe(url: string, done: (loaded: boolean) => void): Unsubscribe;
}
