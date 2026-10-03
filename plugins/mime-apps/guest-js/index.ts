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
	| 'typeInfo'
	| 'handlers'
	| 'openWith'
	| 'openDefault'
	| 'setDefault'
	| 'chooser'
	| 'appIcons'
	| 'typeIcons'
	| 'folderIcons';

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

/** The folders that have an icon of their own besides the plain one: the user's standard folders. */
export type FolderKind =
	| 'plain'
	| 'home'
	| 'desktop'
	| 'documents'
	| 'downloads'
	| 'pictures'
	| 'music'
	| 'videos'
	| 'templates'
	| 'public';

/** What a file or folder icon is for: a content type, a file name extension (without its dot) or a kind of folder. */
export type TypeIconTarget = { mime: string } | { extension: string } | { folder: FolderKind };

export interface TypeIconOptions {
	/** The edge in CSS pixels, 16 to 256. Default 32. */
	size?: number;
	/** The device pixel ratio, 1 to 3. Default 1. */
	scale?: number;
	/** The icon theme to draw from (Linux): the name the system reports. Leave it out for the one in force. It is part of the address, so a change of theme is a new picture. */
	theme?: string | null;
	/** Changes the address without changing the picture: raise it after `refreshTypeIcons` so the webview does not show a picture it kept. */
	revision?: number;
}

/**
 * The address of the icon the system shows for a type of file or a kind of folder, for an `<img src>`, served by the
 * `typeicon://` scheme and keyed by type, never by file: a listing needs one request per distinct type. The picture is a
 * PNG of `size * scale` pixels, drawn from the user's icon theme (Linux) or the shell (Windows); it is a 404 when the
 * system has none, so keep your own icon underneath. Check the `typeIcons` and `folderIcons` features first.
 */
export function typeIconUrl(target: TypeIconTarget, options: TypeIconOptions = {}): string {
	const base = isWindowsWebview() ? 'http://typeicon.localhost' : 'typeicon://localhost';
	const [kind, value] =
		'mime' in target
			? ['mime', target.mime]
			: 'extension' in target
				? ['ext', target.extension]
				: ['folder', target.folder];
	const params = [`size=${options.size ?? 32}`, `scale=${options.scale ?? 1}`];
	if (options.theme) params.push(`theme=${encodeURIComponent(options.theme)}`);
	if (options.revision) params.push(`v=${options.revision}`);
	return `${base}/${kind}/${encodeURIComponent(value)}?${params.join('&')}`;
}

/** Forgets every file and folder icon the plugin made and what the platform kept for them. Call it when the system's icon theme changes, then ask again with a higher `revision`. */
export async function refreshTypeIcons(): Promise<void> {
	await cmd<void>('refresh_type_icons');
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
