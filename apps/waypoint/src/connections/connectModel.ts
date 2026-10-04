// The Connect dialog's pure rules: the form and its draft, what each refusal and connection error says, and which question an error asks
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { AuthMethod } from '@liminal-hq/waypoint-protocol/generated/AuthMethod';
import type { ConnectionDraft } from '@liminal-hq/waypoint-protocol/generated/ConnectionDraft';
import type { DraftError } from '@liminal-hq/waypoint-protocol/generated/DraftError';
import type { KeyringUnavailable } from '@liminal-hq/waypoint-protocol/generated/KeyringUnavailable';
import type { Remembered } from '@liminal-hq/waypoint-protocol/generated/Remembered';
import type { SavedConnection } from '@liminal-hq/waypoint-protocol/generated/SavedConnection';
import type { UnreachableReason } from '@liminal-hq/waypoint-protocol/generated/UnreachableReason';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import { t, tf, type MessageId } from '../i18n/messages';
import { isVfsError } from '../services/vfsClient';
import { isConnectionsRefusal } from './connectionsClient';

/** The dialog's fields, as text. Rust checks them; the page only carries them. */
export interface ConnectForm {
	name: string;
	scheme: string;
	host: string;
	port: string;
	user: string;
	auth: AuthMethod;
	keyFile: string;
	jumpHost: string;
	startFolder: string;
	/** Previews of the server's files (off by default, D14). */
	thumbnails: boolean;
	/** Refresh every so many seconds; blank for never. */
	refreshSeconds: string;
}

/** The field a refusal is about, so its message goes under that field. */
export type FormField =
	| 'address'
	| 'name'
	| 'scheme'
	| 'host'
	| 'port'
	| 'user'
	| 'keyFile'
	| 'jumpHost'
	| 'startFolder'
	| 'refreshSeconds';

export function emptyForm(scheme = 'sftp'): ConnectForm {
	return {
		name: '',
		scheme,
		host: '',
		port: '',
		user: '',
		auth: 'auto',
		keyFile: '',
		jumpHost: '',
		startFolder: '',
		thumbnails: false,
		refreshSeconds: '',
	};
}

/** The fields of a draft Rust read (a typed address) or a saved connection. */
export function formOf(draft: ConnectionDraft | SavedConnection, keep?: ConnectForm): ConnectForm {
	return {
		name: draft.name || keep?.name || '',
		scheme: draft.scheme,
		host: draft.host,
		port: draft.port === null ? '' : String(draft.port),
		user: draft.user ?? '',
		auth: keep && !('id' in draft) ? keep.auth : draft.auth,
		keyFile: draft.keyFile ?? keep?.keyFile ?? '',
		jumpHost: draft.jumpHost ?? keep?.jumpHost ?? '',
		startFolder: draft.startFolder ?? '',
		thumbnails: draft.options.thumbnails,
		refreshSeconds:
			draft.options.refreshSeconds === null ? '' : String(draft.options.refreshSeconds),
	};
}

const blank = (text: string): string | null => (text.trim() === '' ? null : text.trim());

/** A whole number from a field, `null` when blank, and `NaN` when it is not one (Rust refuses it). */
function count(text: string): number | null {
	const trimmed = text.trim();
	if (trimmed === '') return null;
	return /^\d+$/.test(trimmed) ? Number(trimmed) : Number.NaN;
}

/** The draft the form describes. A number that is not one is sent as 0, which Rust refuses by name. */
export function draftOf(form: ConnectForm): ConnectionDraft {
	const port = count(form.port);
	const refresh = count(form.refreshSeconds);
	return {
		name: form.name.trim(),
		scheme: form.scheme,
		host: form.host.trim(),
		port: port === null ? null : Number.isNaN(port) || port > 65535 ? 0 : port,
		user: blank(form.user),
		auth: form.auth,
		keyFile: form.auth === 'keyFile' ? blank(form.keyFile) : null,
		jumpHost: blank(form.jumpHost),
		startFolder: blank(form.startFolder),
		options: {
			thumbnails: form.thumbnails,
			refreshSeconds: refresh === null ? null : Number.isNaN(refresh) ? 0 : refresh,
			timeoutSeconds: null,
			listingRequests: null,
			transferRequests: null,
			windowKib: null,
		},
	};
}

const DRAFT_FIELDS: Record<DraftError['kind'], FormField> = {
	name: 'name',
	scheme: 'scheme',
	host: 'host',
	user: 'user',
	password: 'address',
	port: 'port',
	keyFile: 'keyFile',
	jumpHost: 'jumpHost',
	startFolder: 'startFolder',
	option: 'refreshSeconds',
};

const DRAFT_MESSAGES: Record<DraftError['kind'], MessageId> = {
	name: 'connect.problem.name',
	scheme: 'connect.problem.scheme',
	host: 'connect.problem.host',
	user: 'connect.problem.user',
	password: 'connect.problem.password',
	port: 'connect.problem.port',
	keyFile: 'connect.problem.keyFile',
	jumpHost: 'connect.problem.jumpHost',
	startFolder: 'connect.problem.startFolder',
	option: 'connect.problem.refresh',
};

/** Where a refused save goes in the form, and what it says; `null` for something else. */
export function draftProblem(error: unknown): { field: FormField; message: string } | null {
	if (!isConnectionsRefusal(error)) return null;
	const refusal = error.error;
	switch (refusal.kind) {
		case 'draft':
			return {
				field: DRAFT_FIELDS[refusal.error.kind],
				message: t(DRAFT_MESSAGES[refusal.error.kind]),
			};
		case 'tooMany':
			return { field: 'name', message: t('connect.problem.tooMany') };
		case 'notFound':
		case 'duplicate':
			return { field: 'name', message: t('connect.problem.gone') };
	}
}

const UNREACHABLE: Record<UnreachableReason, MessageId> = {
	nameNotResolved: 'connect.error.nameNotResolved',
	refused: 'connect.error.refused',
	noRoute: 'connect.error.noRoute',
	offline: 'connect.error.offline',
};

/** The question an error asks the person, when it asks one. */
export type Question = 'signIn' | 'hostKey' | 'hostKeyChanged' | 'certificate';

export function questionOf(error: VfsError): Question | null {
	switch (error.kind) {
		case 'authRequired':
		case 'authFailed':
			return 'signIn';
		case 'hostKeyUnknown':
			return 'hostKey';
		case 'hostKeyChanged':
			return 'hostKeyChanged';
		case 'certificateUntrusted':
			return 'certificate';
		default:
			return null;
	}
}

/** Why a connection did not work, in words: the typed reason (D148, SPEC §5.5). */
export function connectionErrorText(error: unknown): string {
	if (!isVfsError(error)) {
		return tf('connect.error.other', {
			detail: error instanceof Error ? error.message : String(error),
		});
	}
	switch (error.kind) {
		case 'unreachable':
			return t(UNREACHABLE[error.reason]);
		case 'timeout':
			return t('connect.error.timeout');
		case 'disconnected':
			return t('connect.error.disconnected');
		case 'authRequired':
			return t('connect.error.authRequired');
		case 'authFailed':
			return t('connect.error.authFailed');
		case 'hostKeyUnknown':
			return t('connect.error.hostKeyUnknown');
		case 'hostKeyChanged':
			return t('connect.error.hostKeyChanged');
		case 'certificateUntrusted':
			return t('connect.error.certificate');
		case 'unsupported':
			return tf('connect.error.unsupported', { what: error.what });
		case 'invalidLocation':
			return t('connect.error.invalid');
		case 'permissionDenied':
			return t('connect.error.permissionDenied');
		case 'notFound':
			return t('connect.error.notFound');
		case 'cancelled':
			return t('connect.error.cancelled');
		default:
			return tf('connect.error.other', { detail: error.kind });
	}
}

const KEYRING: Record<KeyringUnavailable, MessageId> = {
	noKeyring: 'connect.keyring.noKeyring',
	locked: 'connect.keyring.locked',
	failed: 'connect.keyring.failed',
};

/** Why a login cannot be remembered, in words. */
export function keyringText(why: KeyringUnavailable): string {
	return t(KEYRING[why]);
}

/** What to say once a login connected, about "Remember"; `null` when there is nothing to say. */
export function rememberedText(remembered: Remembered): string | null {
	switch (remembered.kind) {
		case 'kept':
			return t('connect.remembered.kept');
		case 'sessionOnly':
			return tf('connect.remembered.sessionOnly', { reason: keyringText(remembered.why) });
		case 'no':
			return null;
	}
}

/** The protocol's name for people. */
export function schemeLabel(scheme: string): string {
	switch (scheme) {
		case 'sftp':
			return t('connect.scheme.sftp');
		case 'smb':
			return t('connect.scheme.smb');
		case 'davs':
			return t('connect.scheme.davs');
		case 'dav':
			return t('connect.scheme.dav');
		case 's3':
			return t('connect.scheme.s3');
		default:
			return scheme;
	}
}
