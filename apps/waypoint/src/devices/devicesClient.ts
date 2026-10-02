// What the sidebar's Devices section needs from the volumes plugin: the list, the actions and live changes
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { PluginStatus, Volume, VolumesChanged } from '@liminal-hq/plugin-volumes';
import type { Unsubscribe } from '../services/vfsClient';

/**
 * The seam between the Devices section and the volumes plugin, injected so the section is tested
 * without it. Rust owns the volumes (A62); this only asks, acts and listens.
 *
 * Every action rejects with the plugin's `VolumesError` (`isVolumesError` tells it from anything
 * else), which `deviceModel.ts` turns into a sentence.
 */
export interface DevicesClient {
	/** Which features work here, each with a reason when it does not. */
	getStatus(): Promise<PluginStatus>;
	/** Every volume now. */
	list(): Promise<Volume[]>;
	/** Mounts a volume and resolves with its mount point. */
	mount(id: string): Promise<string>;
	unmount(id: string): Promise<void>;
	eject(id: string): Promise<void>;
	/** Unlocks an encrypted volume and resolves with the id of the volume that appears. The passphrase is never kept. */
	unlock(id: string, passphrase: string): Promise<string>;
	/** Hears the whole list after each change, with a revision that only grows. */
	onChanged(listener: (event: VolumesChanged) => void): Unsubscribe;
}
