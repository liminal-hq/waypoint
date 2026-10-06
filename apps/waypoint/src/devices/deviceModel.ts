// What the Devices section shows of a volume, and the sentence for a failed action, as pure functions
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import {
	hasFeature,
	isVolumesError,
	type PluginStatus,
	type Reason,
	type Volume,
	type VolumesError,
} from '@liminal-hq/plugin-volumes';
import { formatSize } from '../browse/format';
import { t, tf, type MessageId } from '../i18n/messages';

/** The actions a volume's row can offer. */
export type DeviceAction = 'mount' | 'unmount' | 'eject' | 'unlock' | 'forget';

/** From this share used, a bar is marked "almost full" in words and in its pattern, not only its colour. */
export const ALMOST_FULL_PERCENT = 90;

/** What a volume's usage bar and text say; `null` where the size or the free space is not known. */
export interface Usage {
	/** Whole percent used, 0 to 100. */
	percent: number;
	almostFull: boolean;
	/** "120 GB free of 500 GB", and ". Almost full" appended when it is. */
	text: string;
	/** "120 GB free of 500 GB" alone, so the warning can sit on a line of its own. */
	summary: string;
	/** "Almost full" when it is, else `null`. */
	warning: string | null;
}

export function usageOf(volume: Volume): Usage | null {
	const { total, free } = volume;
	if (total === null || free === null || total <= 0) return null;
	const used = Math.min(total, Math.max(0, total - free));
	const percent = Math.round((used / total) * 100);
	const almostFull = percent >= ALMOST_FULL_PERCENT;
	const base = tf('devices.space', { free: formatSize(free), total: formatSize(total) });
	const warning = almostFull ? t('devices.almostFull') : null;
	return {
		percent,
		almostFull,
		text: warning ? `${base}. ${warning}` : base,
		summary: base,
		warning,
	};
}

/** The one line under a volume's name: its space, or its state when there is no space to show. */
export function statusText(volume: Volume): string {
	if (volume.locked) return t('devices.state.locked');
	const usage = usageOf(volume);
	if (usage) return usage.text;
	if (!volume.mountPoint) {
		return volume.total !== null
			? tf('devices.state.unmountedSized', { total: formatSize(volume.total) })
			: t('devices.state.unmounted');
	}
	return t('devices.spaceUnknown');
}

/**
 * The volumes worth a row: what is mounted, locked, or can be mounted by the person. The system's
 * own unmounted partitions (the boot partition, swap, recovery) are left out; they are not places.
 */
export function visibleVolumes(volumes: readonly Volume[]): Volume[] {
	return volumes.filter(
		(volume) =>
			volume.mountPoint !== null || volume.locked || (volume.canMount && !volume.isSystem),
	);
}

/** What the Unlock dialog says about remembering the passphrase (D147, D153). */
export type RememberOffer =
	| { kind: 'ask' }
	/** The person turned it on but it cannot work now: the dialog says why. */
	| { kind: 'unavailable'; reason: 'no-keyring' | 'keyring-locked' };

/**
 * Whether to offer "Remember in keyring": the checkbox when the plugin says it works, a line of
 * words when the person has it switched on but there is no keyring or it is locked, and nothing
 * where it is off in Settings or cannot exist here (the Services panel says why).
 */
export function rememberOffer(status: PluginStatus | null): RememberOffer | null {
	if (!status || !hasFeature(status, 'unlock')) return null;
	if (hasFeature(status, 'remember')) return { kind: 'ask' };
	const reason = status.features.find((feature) => feature.name === 'remember')?.reason;
	if (reason === 'no-keyring' || reason === 'keyring-locked')
		return { kind: 'unavailable', reason };
	return null;
}

const REMEMBER_REASONS: Partial<Record<Reason, MessageId>> = {
	'no-keyring': 'devices.remember.reason.noKeyring',
	'keyring-locked': 'devices.remember.reason.keyringLocked',
};

/** The short phrase for why a passphrase could not be remembered, from the plugin's reason code. */
export function rememberReasonText(reason: Reason, message: string): string {
	const id = REMEMBER_REASONS[reason];
	return id ? t(id) : message;
}

/** The actions to offer for `volume`: those it allows and the plugin reports as working here. */
export function actionsFor(volume: Volume, status: PluginStatus | null): DeviceAction[] {
	if (!status) return [];
	const actions: DeviceAction[] = [];
	const forget = volume.remembered && hasFeature(status, 'remember');
	if (volume.locked) {
		if (hasFeature(status, 'unlock')) actions.push('unlock');
		if (forget) actions.push('forget');
		return actions;
	}
	if (volume.canMount && !volume.mountPoint && hasFeature(status, 'mount')) actions.push('mount');
	if (volume.canUnmount && volume.mountPoint && hasFeature(status, 'unmount'))
		actions.push('unmount');
	if (volume.canEject && hasFeature(status, 'eject')) actions.push('eject');
	if (forget) actions.push('forget');
	return actions;
}

const VERBS: Record<DeviceAction, MessageId> = {
	mount: 'devices.verb.mount',
	unmount: 'devices.verb.unmount',
	eject: 'devices.verb.eject',
	unlock: 'devices.verb.unlock',
	forget: 'devices.verb.forget',
};

/** The sentence for a failed action: the reason, and what holds the volume when the plugin says (D120). */
export function failureText(action: DeviceAction, name: string, error: unknown): string {
	const values = { action: t(VERBS[action]), name };
	if (!isVolumesError(error)) return tf('devices.error.unknown', values);
	return describe(error, values);
}

function describe(error: VolumesError, values: { action: string; name: string }): string {
	switch (error.kind) {
		case 'busy':
			return error.by
				? tf('devices.error.busyBy', { ...values, by: error.by })
				: tf('devices.error.busy', values);
		case 'notAuthorised':
			return tf('devices.error.notAuthorised', values);
		case 'wrongPassphrase':
			return t('devices.unlock.wrong');
		case 'unsupported':
			return tf('devices.error.unsupported', values);
		case 'notFound':
			return tf('devices.error.notFound', values);
		case 'io':
			return tf('devices.error.io', { ...values, message: error.message });
	}
}
