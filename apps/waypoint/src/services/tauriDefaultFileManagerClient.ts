// The real DefaultFileManagerClient, over the mime-apps plugin's guest-js API
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import {
	featureMessage,
	featureReason,
	getStatus,
	handlers,
	hasFeature,
	isMimeAppsError,
	openDefaultAppsSettings,
	setDefault,
} from '@liminal-hq/plugin-mime-apps';
import {
	DIRECTORY_TYPE,
	WAYPOINT_DESKTOP_ID,
	type CurrentFileManager,
	type DefaultFileManagerClient,
	type FileManagerAction,
} from './defaultFileManagerClient';

/** Any folder will do to ask who opens folders; the root exists everywhere. */
const SOME_FOLDER = 'file:///';

/** The sentence for a refused or failed call. */
function sentence(error: unknown): string {
	if (isMimeAppsError(error)) {
		switch (error.kind) {
			case 'appNotFound':
				return 'Waypoint is not installed as an application on this system, so it cannot be made the default. Install the package, then try again.';
			case 'unsupported':
				return 'This system does not let applications change the default.';
			case 'failed':
				return error.message;
			default:
				return error.kind;
		}
	}
	return error instanceof Error ? error.message : String(error);
}

/** A `DefaultFileManagerClient` over `mime-apps`. */
export function createTauriDefaultFileManagerClient(): DefaultFileManagerClient {
	return {
		async action(): Promise<FileManagerAction> {
			const status = await getStatus();
			if (hasFeature(status, 'setDefault')) return { kind: 'set' };
			// Windows leaves the choice to the person and offers its Default apps page instead.
			if (featureReason(status, 'setDefault') === 'managed-by-system') return { kind: 'settings' };
			return {
				kind: 'unavailable',
				reason: featureMessage(status, 'setDefault') ?? 'This system does not offer it.',
			};
		},
		async current(): Promise<CurrentFileManager | null> {
			try {
				const found = await handlers([SOME_FOLDER]);
				if (found.mime !== DIRECTORY_TYPE) return null;
				return {
					isWaypoint: found.default?.id === WAYPOINT_DESKTOP_ID,
					name:
						found.default && found.default.id !== WAYPOINT_DESKTOP_ID ? found.default.name : null,
				};
			} catch {
				return null;
			}
		},
		async make(): Promise<void> {
			const status = await getStatus();
			try {
				if (hasFeature(status, 'setDefault')) {
					await setDefault(DIRECTORY_TYPE, WAYPOINT_DESKTOP_ID);
				} else {
					await openDefaultAppsSettings();
				}
			} catch (error) {
				throw new Error(sentence(error));
			}
		},
	};
}
