// Exposes typed guest-side wrappers for the mime-apps plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import type { App } from './bindings/App';
import type { FeatureStatus } from './bindings/FeatureStatus';
import type { Flavour } from './bindings/Flavour';
import type { Handlers } from './bindings/Handlers';
import type { MimeAppsError } from './bindings/MimeAppsError';
import type { PluginStatus } from './bindings/PluginStatus';
import type { Reason } from './bindings/Reason';
import type { TypeInfo } from './bindings/TypeInfo';

export type {
	App,
	FeatureStatus,
	Flavour,
	Handlers,
	MimeAppsError,
	PluginStatus,
	Reason,
	TypeInfo,
};

const PREFIX = 'plugin:mime-apps|';

/** The names `getStatus().features` uses. */
export type Feature =
	'typeInfo' | 'handlers' | 'openWith' | 'openDefault' | 'setDefault' | 'chooser' | 'appIcons';

function cmd<T>(name: string, args?: Record<string, unknown>): Promise<T> {
	return invoke<T>(`${PREFIX}${name}`, args);
}

/** Reports which features work on this system, each with a typed reason when it does not, and which implementation is behind them. */
export function getStatus(): Promise<PluginStatus> {
	return cmd<PluginStatus>('get_status');
}

/** True if the feature is available. Decide behaviour from the features, never from the platform. */
export function hasFeature(status: PluginStatus, feature: Feature): boolean {
	return status.features.some((entry) => entry.name === feature && entry.available);
}

/** Why a feature is unavailable, as a code to branch on (`flatpak-sandbox`, `no-system-chooser`, `managed-by-system`, …), or `undefined` when it works. */
export function featureReason(status: PluginStatus, feature: Feature): Reason | undefined {
	return status.features.find((entry) => entry.name === feature)?.reason ?? undefined;
}

/** A sentence that explains why a feature is unavailable, or `undefined` when it works. */
export function featureMessage(status: PluginStatus, feature: Feature): string | undefined {
	return status.features.find((entry) => entry.name === feature)?.message ?? undefined;
}

/**
 * The type of a path or URI (`file:///…`, `smb://…`, a plain absolute path): its type, a phrase for it and an icon name.
 * A name ending in a slash, or a local directory, is `inode/directory`. The start of a local file is read only when
 * `sniff` is true.
 */
export function typeInfo(uri: string, sniff = false): Promise<TypeInfo> {
	return cmd<TypeInfo>('type_info', { uri, sniff });
}

/**
 * The applications for the locations' type: the default, the ones registered for it and the others that can open it.
 * For locations of different types only the applications that open all of them are listed and `mixed` is true.
 */
export function handlers(uris: string[]): Promise<Handlers> {
	return cmd<Handlers>('handlers', { uris });
}

/** Opens the locations in the application with this id (an `App.id`). Rejects with a `MimeAppsError`. */
export async function openWith(uris: string[], appId: string): Promise<void> {
	await cmd<void>('open_with', { uris, appId });
}

/** Opens each location in its default application. Rejects with `{ kind: 'noHandler', mime }` when a type has none; nothing is opened then. */
export async function openDefault(uris: string[]): Promise<void> {
	await cmd<void>('open_default', { uris });
}

/**
 * Asks the system to let the person choose an application (on Windows the Open With dialog, as a child of the window
 * labelled `parentLabel`; in a Flatpak sandbox the portal's chooser). Rejects with `{ kind: 'cancelled' }` when it is
 * dismissed and `{ kind: 'unsupported' }` when the system has no chooser: draw a list from `handlers` then.
 */
export async function choose(uris: string[], parentLabel?: string): Promise<void> {
	await cmd<void>('choose', { uris, parentLabel });
}

/** Makes an application the default for a type (`image/png`). Rejects with `{ kind: 'unsupported' }` where the system does not allow it. */
export async function setDefault(mime: string, appId: string): Promise<void> {
	await cmd<void>('set_default', { mime, appId });
}

/** Opens the system's own page for default applications (Windows Settings). Rejects with `{ kind: 'unsupported' }` elsewhere. */
export async function openDefaultAppsSettings(): Promise<void> {
	await cmd<void>('open_default_apps_settings');
}

/**
 * The address of an application's icon, for an `<img src>`, served by the `appicon://` scheme by app id only. The
 * picture is a PNG at about `size` pixels (16 to 256); it is a 404 when the id is unknown or has no icon.
 */
export function appIconUrl(appId: string, size = 32): string {
	const base = isWindowsWebview() ? 'http://appicon.localhost' : 'appicon://localhost';
	return `${base}/${encodeURIComponent(appId)}?size=${size}`;
}

function isWindowsWebview(): boolean {
	return typeof navigator !== 'undefined' && /Windows/i.test(navigator.userAgent);
}

/** True if a rejected value is a `MimeAppsError`. */
export function isMimeAppsError(value: unknown): value is MimeAppsError {
	return (
		typeof value === 'object' && value !== null && typeof (value as MimeAppsError).kind === 'string'
	);
}
