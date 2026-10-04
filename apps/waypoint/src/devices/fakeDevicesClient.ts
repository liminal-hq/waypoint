// An in-memory DevicesClient for building and testing the Devices section without the plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type {
	PluginStatus,
	Reason,
	RememberOutcome,
	Unlocked,
	Volume,
	VolumesChanged,
	VolumesError,
} from '@liminal-hq/plugin-volumes';
import type { Unsubscribe } from '../services/vfsClient';
import type { DevicesClient } from './devicesClient';

/** A volume with every field filled in; `overrides` change what a test cares about. */
export function fakeVolume(id: string, overrides: Partial<Volume> = {}): Volume {
	return {
		id,
		label: id,
		kind: 'removable',
		fileSystem: 'ext4',
		mountPoint: `/run/media/test/${id}`,
		uri: null,
		total: 500_000_000_000,
		free: 200_000_000_000,
		canMount: false,
		canUnmount: true,
		canEject: true,
		canPowerOff: false,
		locked: false,
		isSystem: false,
		device: `/dev/${id}`,
		uuid: null,
		remembered: false,
		...overrides,
	};
}

/** A status in which every feature works, or with `unavailable` features reported as missing. */
export function fakeStatus(
	unavailable: readonly string[] = [],
	/** Whether remembering passphrases works: `on`, or the reason it does not (`disabled`, the default, is the Settings switch being off). */
	remember: 'on' | Reason = 'disabled',
): PluginStatus {
	const features = ['list', 'mount', 'unmount', 'eject', 'unlock', 'watch'].map((name) => ({
		name,
		available: !unavailable.includes(name),
		reason: unavailable.includes(name) ? ('not-supported' as const) : null,
		message: unavailable.includes(name) ? `${name} is not supported here` : null,
	}));
	features.push({
		name: 'remember',
		available: remember === 'on',
		reason: remember === 'on' ? null : (remember as 'not-supported'),
		message: remember === 'on' ? null : `remembering is unavailable (${remember})`,
	});
	return {
		available: features.some((feature) => feature.available),
		reason: null,
		message: null,
		flavour: 'udisks2',
		features,
	};
}

/** What an action did, for a test to read. */
export interface FakeCall {
	action: 'mount' | 'unmount' | 'eject' | 'unlock' | 'forget' | 'refreshSpace';
	id: string;
	passphrase?: string;
	remember?: boolean;
}

/**
 * Holds a list of volumes and plays the plugin: actions change it as the system would and send
 * `volumes://changed`; `failNext` makes the next action reject, and `hold` keeps the next one
 * pending until `release`.
 */
export class FakeDevicesClient implements DevicesClient {
	status: PluginStatus;
	readonly calls: FakeCall[] = [];
	listCalls = 0;
	private volumes: Volume[];
	private revision = 1;
	private listeners = new Set<(event: VolumesChanged) => void>();
	private failure: VolumesError | null = null;
	private gate: Promise<void> | null = null;
	private open: (() => void) | null = null;
	/** The passphrase that unlocks, per locked volume id; any other rejects with `wrongPassphrase`. */
	passphrases = new Map<string, string>();
	/** What a request to remember comes to, when the passphrase is right; `remembered` also marks the volume. */
	rememberOutcome: RememberOutcome = { state: 'remembered' };

	constructor(volumes: Volume[] = [], status: PluginStatus = fakeStatus()) {
		this.volumes = volumes;
		this.status = status;
	}

	async getStatus(): Promise<PluginStatus> {
		return this.status;
	}

	async list(): Promise<Volume[]> {
		this.listCalls += 1;
		return structuredClone(this.volumes);
	}

	/** The `free` and `total` a network volume reports once measured, by id; a volume not in the map stays unmeasured. */
	measurements = new Map<string, { total: number; free: number }>();

	async refreshSpace(id: string): Promise<Volume> {
		this.calls.push({ action: 'refreshSpace', id });
		await this.begin(null);
		const measured = this.measurements.get(id);
		if (measured) this.replace(id, measured);
		return structuredClone(this.find(id));
	}

	async mount(id: string): Promise<string> {
		await this.begin({ action: 'mount', id });
		const volume = this.find(id);
		const mountPoint = `/run/media/test/${volume.label}`;
		this.replace(id, { mountPoint, canMount: false, canUnmount: true });
		return mountPoint;
	}

	async unmount(id: string): Promise<void> {
		await this.begin({ action: 'unmount', id });
		this.replace(id, { mountPoint: null, canMount: true, canUnmount: false, free: null });
	}

	async eject(id: string): Promise<void> {
		await this.begin({ action: 'eject', id });
		this.remove(id);
	}

	async unlock(id: string, passphrase: string, remember = false): Promise<Unlocked> {
		await this.begin({ action: 'unlock', id, passphrase, remember });
		if (this.passphrases.get(id) !== passphrase) throw { kind: 'wrongPassphrase' } as VolumesError;
		const opened = `${id}-open`;
		const locked = this.find(id);
		this.volumes = this.volumes.map((volume) =>
			volume.id === id ? { ...volume, locked: false } : volume,
		);
		this.volumes.push(
			fakeVolume(opened, {
				label: locked.label,
				mountPoint: null,
				canMount: true,
				canUnmount: false,
				uuid: locked.uuid,
				remembered: remember && this.rememberOutcome.state === 'remembered',
			}),
		);
		this.publish();
		return {
			id: opened,
			remember: remember ? this.rememberOutcome : { state: 'notAsked' },
		};
	}

	async forget(id: string): Promise<boolean> {
		await this.begin({ action: 'forget', id });
		const had = this.find(id).remembered;
		this.replace(id, { remembered: false });
		return had;
	}

	onChanged(listener: (event: VolumesChanged) => void): Unsubscribe {
		this.listeners.add(listener);
		return () => {
			this.listeners.delete(listener);
		};
	}

	/** Makes the next action reject with `error`. */
	failNext(error: VolumesError): void {
		this.failure = error;
	}

	/** Keeps the next action pending until `release` is called. */
	hold(): void {
		this.gate = new Promise((resolve) => {
			this.open = resolve;
		});
	}

	release(): void {
		this.open?.();
		this.open = null;
	}

	/** Plugs a volume in, as the system would, and sends the change. */
	plug(volume: Volume): void {
		this.volumes = [...this.volumes, volume];
		this.publish();
	}

	/** Takes a volume away without anyone asking, as pulling a stick out does. */
	unplug(id: string): void {
		this.remove(id);
	}

	/** Changes a volume from outside, as another program mounting it would. */
	change(id: string, overrides: Partial<Volume>): void {
		this.replace(id, overrides);
	}

	/** Sends the current list again with a revision that has not grown, as a stale event would arrive. */
	sendStale(): void {
		const event: VolumesChanged = { revision: this.revision - 1, volumes: [] };
		for (const listener of [...this.listeners]) listener(event);
	}

	private async begin(call: FakeCall | null): Promise<void> {
		if (call) this.calls.push(call);
		if (this.gate) {
			const gate = this.gate;
			this.gate = null;
			await gate;
		}
		if (this.failure) {
			const error = this.failure;
			this.failure = null;
			throw error;
		}
	}

	private find(id: string): Volume {
		const volume = this.volumes.find((entry) => entry.id === id);
		if (!volume) throw { kind: 'notFound' } as VolumesError;
		return volume;
	}

	private replace(id: string, overrides: Partial<Volume>): void {
		this.find(id);
		this.volumes = this.volumes.map((volume) =>
			volume.id === id ? { ...volume, ...overrides } : volume,
		);
		this.publish();
	}

	private remove(id: string): void {
		this.volumes = this.volumes.filter((volume) => volume.id !== id);
		this.publish();
	}

	private publish(): void {
		this.revision += 1;
		const event: VolumesChanged = {
			revision: this.revision,
			volumes: structuredClone(this.volumes),
		};
		for (const listener of [...this.listeners]) listener(event);
	}
}
