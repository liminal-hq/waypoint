// Whether Administrator access is on and working in this window: the Experimental setting and the elevate plugin's status
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { getStatus, hasFeature, type PluginStatus } from '@liminal-hq/plugin-elevate';
import { createContext, useContext, useEffect, useState, type ReactNode } from 'react';
import { useSettings } from '../settings/SettingsContext';

/** Reads the elevate plugin's status. The window supplies the plugin's own function; a test supplies its own. */
export type ElevateStatusReader = () => Promise<PluginStatus>;

const ElevationContext = createContext<ElevateStatusReader>(getStatus);

/** Whether `status` says a helper can be started on this system. */
export function elevationWorks(status: PluginStatus): boolean {
	return status.available && hasFeature(status, 'elevate');
}

/** Supplies the reader of the elevate plugin's status to the views below it. */
export function ElevationProvider({
	status,
	children,
}: {
	status: ElevateStatusReader;
	children: ReactNode;
}) {
	return <ElevationContext.Provider value={status}>{children}</ElevationContext.Provider>;
}

/**
 * True when Settings → Experimental → Administrator access is on and the plugin reports that a
 * helper can be started. The status is read when the setting is turned on and not while it is off,
 * so a window that never uses the feature never asks; a status that cannot be read is unavailable.
 */
export function useElevationAvailable(): boolean {
	const on = useSettings((settings) => settings.experimental.administratorAccess);
	const read = useContext(ElevationContext);
	const [works, setWorks] = useState(false);
	useEffect(() => {
		if (!on) {
			setWorks(false);
			return;
		}
		let live = true;
		read().then(
			(status) => {
				if (live) setWorks(elevationWorks(status));
			},
			(error: unknown) => {
				console.warn('could not read the administrator access status', error);
				if (live) setWorks(false);
			},
		);
		return () => {
			live = false;
		};
	}, [on, read]);
	return on && works;
}
