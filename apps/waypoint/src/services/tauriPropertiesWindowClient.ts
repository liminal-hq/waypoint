// The real PropertiesWindowClient: the app's own commands for the Properties windows
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { OpenOutcome, PropertiesWindowClient } from './propertiesWindowClient';

/** A `PropertiesWindowClient` over the app's commands. */
export function createTauriPropertiesWindowClient(): PropertiesWindowClient {
	return {
		open: (location: Location) => invoke<OpenOutcome>('open_properties_window', { location }),
		subject: () => invoke<Location>('properties_subject'),
		setSubject: (location: Location) => invoke<void>('properties_set_subject', { location }),
	};
}
