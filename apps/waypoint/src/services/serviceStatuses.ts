// The native plugins whose availability the Services panel reports, adapted to the shared status shape
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { getStatus as mimeAppsStatus } from '@liminal-hq/plugin-mime-apps';
import { getStatus as nativeDndStatus } from '@liminal-hq/plugin-native-dnd';
import { getStatus as osPrefsStatus } from '@liminal-hq/plugin-os-prefs';
import { getStatus as systemAppearanceStatus } from '@liminal-hq/plugin-system-appearance';
import { getStatus as secretsStatus } from '@liminal-hq/plugin-secrets';
import { getStatus as thumbnailsStatus } from '@liminal-hq/plugin-thumbnails';
import { getStatus as trashStatus } from '@liminal-hq/plugin-trash';
import { getStatus as volumesStatus } from '@liminal-hq/plugin-volumes';
import { getStatus as windowEffectsStatus } from '@liminal-hq/plugin-window-effects';
import { getStatus as windowManagerStatus } from '@liminal-hq/plugin-window-manager';
import { getStatus as windowTearoffStatus } from '@liminal-hq/plugin-window-tearoff';
import { getStatus as gitStatus } from '@liminal-hq/waypoint-plugin-git';
import { getStatus as opsStatus } from '@liminal-hq/waypoint-plugin-ops';
import { getStatus as sessionStatus } from '@liminal-hq/waypoint-plugin-session';
import { getStatus as settingsStatus } from '@liminal-hq/waypoint-plugin-settings';
import { connectionSupport, getStatus as vfsStatus } from '@liminal-hq/waypoint-plugin-vfs';
import type { PluginStatus } from '@liminal-hq/waypoint-protocol/generated/PluginStatus';
import { t } from '../i18n/messages';
import { collectStatuses, type StatusSource } from './status';
import { createTauriIntegrationsClient } from './tauriIntegrationsClient';
import { protocolDetails } from './tauriProtocolsClient';

/** One feature of a reusable plugin: whether it works and, when it does not, why. */
interface FeatureReport {
	available: boolean;
	reason: string | null;
}

/** The feature names that work, and the first reason one that does not (or works only partly) gives. */
function summarise(
	status: { available: boolean; reason: string | null },
	features: ReadonlyArray<readonly [string, FeatureReport]>,
): PluginStatus {
	const working = features.filter(([, report]) => report.available).map(([name]) => name);
	const reason =
		status.reason ?? features.find(([, report]) => !report.available && report.reason)?.[1].reason;
	return { available: status.available, reason: reason ?? null, features: working };
}

/**
 * The Trash plugin's status in the shared shape. It stays available when it can only move files to
 * the Trash (a sandbox's portal), with the features it lacks (`list`, `restore`, `empty`, `expiry`)
 * missing from the list and the first reason given, which is what the Trash place explains.
 */
export async function trashServiceStatus(): Promise<PluginStatus> {
	const status = await trashStatus();
	return summarise(
		status,
		status.features.map((feature) => [feature.name, feature] as const),
	);
}

/** The system preferences plugin's status in the shared shape: its features carry their own reasons. */
export async function osPrefsServiceStatus(): Promise<PluginStatus> {
	const status = await osPrefsStatus();
	return summarise(
		status,
		status.features.map((feature) => [feature.name, feature] as const),
	);
}

/** The thumbnails plugin's status in the shared shape: its features carry their own reasons. */
export async function thumbnailsServiceStatus(): Promise<PluginStatus> {
	const status = await thumbnailsStatus();
	return summarise(
		{ available: status.available, reason: status.reason?.message ?? null },
		status.features.map(
			(feature) =>
				[
					feature.name,
					{ available: feature.available, reason: feature.reason?.message ?? null },
				] as const,
		),
	);
}

/** The reason codes of a feature that is simply not in use (remembering passphrases switched off, or not offered by the app). */
const QUIET_REASONS: ReadonlyArray<string> = ['disabled', 'not-configured'];

/** The volumes plugin's status in the shared shape: its features carry typed reasons, shown as the sentence they hold. */
export async function volumesServiceStatus(): Promise<PluginStatus> {
	const status = await volumesStatus();
	return summarise(
		{ available: status.available, reason: status.message },
		status.features.map(
			(feature) =>
				[
					feature.name,
					{
						available: feature.available,
						// Remembering passphrases is off until the person turns it on (D153): not a fault to show.
						reason:
							feature.reason && QUIET_REASONS.includes(feature.reason) ? null : feature.message,
					},
				] as const,
		),
	);
}

/** The keyring plugin's status in the shared shape: its features carry typed reasons, shown as the sentence they hold (no keyring running, or locked). */
export async function secretsServiceStatus(): Promise<PluginStatus> {
	const status = await secretsStatus();
	return summarise(
		{ available: status.available, reason: status.message },
		status.features.map(
			(feature) =>
				[feature.name, { available: feature.available, reason: feature.message }] as const,
		),
	);
}

/** The reason codes of features that belong to another platform (Mica on Linux, the shadow inset on Windows): not a fault to show. */
const FOREIGN_REASONS: ReadonlyArray<string> = ['windows-only', 'gtk-only'];

/** The window effects plugin's status in the shared shape: its features carry typed reasons, shown as the sentence they hold, except for the ones that simply belong to another platform. */
export async function windowEffectsServiceStatus(): Promise<PluginStatus> {
	const status = await windowEffectsStatus();
	return summarise(
		{ available: status.available, reason: status.message },
		status.features.map(
			(feature) =>
				[
					feature.name,
					{
						available: feature.available,
						reason:
							feature.reason && FOREIGN_REASONS.includes(feature.reason) ? null : feature.message,
					},
				] as const,
		),
	);
}

/** The mime-apps plugin's status in the shared shape: its features carry typed reasons, shown as the sentence they hold. */
export async function mimeAppsServiceStatus(): Promise<PluginStatus> {
	const status = await mimeAppsStatus();
	return summarise(
		{ available: status.available, reason: status.message },
		status.features.map(
			(feature) =>
				[feature.name, { available: feature.available, reason: feature.message }] as const,
		),
	);
}

/**
 * The system appearance plugin's status in the shared shape. Its own `available`, `reason` and
 * `features` describe the titlebar preferences only, so the plugin counts as available when the
 * titlebar or any appearance preference works, and lists the titlebar sources and the appearance
 * features that work, with `palette` (the colour palette, which "Match the system's colours" needs)
 * among them when it works; the first reason is the titlebar's, then the first appearance feature's
 * (or the palette's) that does not work.
 */
export async function systemAppearanceServiceStatus(): Promise<PluginStatus> {
	const status = await systemAppearanceStatus();
	const summary = summarise(
		{
			available: status.available || status.appearanceAvailable || status.palette.available,
			reason: status.reason,
		},
		[
			...status.appearance.map(
				(feature) =>
					[feature.feature, { available: feature.available, reason: feature.detail }] as const,
			),
			['palette', { available: status.palette.available, reason: status.palette.detail }] as const,
		],
	);
	return { ...summary, features: [...status.features, ...summary.features] };
}

/** The native drag and drop plugin's status in the shared shape. */
export async function nativeDndServiceStatus(): Promise<PluginStatus> {
	const status = await nativeDndStatus();
	return summarise(status, Object.entries(status.features));
}

/**
 * A remote protocol's line in the Services panel (D167): available while its provider is
 * registered (with what it can do, and what it cannot, in the provider's own words when it has
 * them), "Turned off in Settings → Experimental" while its switch is off, and "Not included in this
 * build" where the build has no provider for it. `schemes` are the URI schemes it serves and `key`
 * names it for `get_protocol_details`.
 */
export function protocolServiceStatus(schemes: readonly string[], key: string): StatusSource {
	return async () => {
		const support = await connectionSupport();
		if (schemes.some((scheme) => support.schemes.includes(scheme))) {
			// What the provider can do is a nicety: the line is available without it.
			const detail = await protocolDetails().then(
				(all) => all[key],
				() => undefined,
			);
			return {
				available: true,
				reason: detail?.reason ?? null,
				features: detail && detail.features.length > 0 ? detail.features : [...schemes],
			};
		}
		const off = schemes.some((scheme) => support.off.includes(scheme));
		return {
			available: false,
			reason: off ? t('protocol.off.reason') : t('services.protocol.notBuilt'),
			features: [],
		};
	};
}

let inflight: Promise<Record<string, PluginStatus>> | null = null;

/**
 * The statuses of the two shared plugins that only Rust uses (A65), asked of the app's own command.
 * Both entries of one load share a single probe, which talks to the session bus.
 */
function integrationStatuses(): Promise<Record<string, PluginStatus>> {
	inflight ??= createTauriIntegrationsClient()
		.statuses()
		.finally(() => {
			inflight = null;
		});
	return inflight;
}

/** One of the two Rust-only plugins' status: a plugin the command did not report is unavailable. */
export function integrationServiceStatus(key: string): StatusSource {
	return async () =>
		(await integrationStatuses())[key] ?? {
			available: false,
			reason: null,
			features: [],
		};
}

/**
 * The plugins the panel reports, each through its own guest-js `getStatus`. Every plugin
 * `src-tauri` registers has an entry (`scripts/check-services.sh` fails the build otherwise); the
 * key is the crate name without `tauri_plugin_`, except the file system's.
 */
export const SERVICE_SOURCES: Record<string, StatusSource> = {
	'file-system': vfsStatus,
	trash: trashServiceStatus,
	'native-dnd': nativeDndServiceStatus,
	// Whether the Shelf window (and every other window) can be kept on top, and why not where it cannot.
	'window-manager': windowManagerStatus,
	'waypoint-ops': opsStatus,
	'waypoint-session': sessionStatus,
	// Reads repositories for the Git column, marks and branch; unavailable, with the reason, while the Settings switch has it off.
	'waypoint-git': gitStatus,
	'waypoint-settings': settingsStatus,
	'os-prefs': osPrefsServiceStatus,
	'system-appearance': systemAppearanceServiceStatus,
	'window-tearoff': windowTearoffStatus,
	thumbnails: thumbnailsServiceStatus,
	volumes: volumesServiceStatus,
	// Saved logins and remembered passphrases: says why when no keyring runs or it stays locked.
	secrets: secretsServiceStatus,
	'window-effects': windowEffectsServiceStatus,
	'mime-apps': mimeAppsServiceStatus,
	// The remote protocols are not plugins: each is a provider the file system plugin serves while its Settings → Experimental switch is on (D167).
	sftp: protocolServiceStatus(['sftp'], 'sftp'),
	smb: protocolServiceStatus(['smb'], 'smb'),
	webdav: protocolServiceStatus(['dav', 'davs'], 'webdav'),
	s3: protocolServiceStatus(['s3'], 's3'),
	// Used only from Rust, so they report through the app's `get_integration_statuses` (A66).
	'xdg-portal': integrationServiceStatus('xdg-portal'),
	'desktop-integration': integrationServiceStatus('desktop-integration'),
};

/** Asks every plugin whether it works here; one that cannot answer is reported unavailable with the error as its reason. */
export function collectServiceStatuses(): Promise<Record<string, PluginStatus>> {
	return collectStatuses(SERVICE_SOURCES);
}
