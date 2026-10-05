// The Connect dialog's pure rules: the form and its draft, what each refusal and connection error says, and which question an error asks
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { RemoteThumbnails } from '@liminal-hq/waypoint-protocol/generated/RemoteThumbnails';
import type { AuthMethod } from '@liminal-hq/waypoint-protocol/generated/AuthMethod';
import type { DavAuth } from '@liminal-hq/waypoint-protocol/generated/DavAuth';
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
import { s3Endpoint, s3Preset, s3PresetOf } from './s3Presets';

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
	/** Previews of the server's files (off by default, D14, D166). */
	thumbnails: RemoteThumbnails;
	/** The largest file read whole for a preview, in MB; blank for the default. */
	thumbnailMaxMb: string;
	/** Refresh every so many seconds; blank for never. */
	refreshSeconds: string;
	/** SMB: the domain the user belongs to; it is joined to the user as `domain;user`. */
	domain: string;
	/** WebDAV: how a password is sent. */
	davAuth: DavAuth;
	/** WebDAV: the server is a Nextcloud (or ownCloud). */
	nextcloud: boolean;
	/** S3: the service (`aws`, `b2`, `r2`, `wasabi`, `minio`, `spaces` or `custom`). */
	s3Preset: string;
	/** S3: what the service needs to find its endpoint: a region, an account id, a host, or the whole endpoint. */
	s3Value: string;
	/** S3: the signing region, when the service does not say it; blank follows the service. */
	s3Region: string;
	/** S3: put the bucket in the path (`true`), in the host name (`false`), or as the service does (`null`). */
	s3PathStyle: boolean | null;
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
	| 'refreshSeconds'
	| 'domain'
	| 's3Value'
	| 's3Region'
	| 'thumbnailMaxMb';

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
		thumbnails: 'off',
		thumbnailMaxMb: '',
		refreshSeconds: '',
		domain: '',
		davAuth: 'auto',
		nextcloud: false,
		s3Preset: 'aws',
		s3Value: '',
		s3Region: '',
		s3PathStyle: null,
	};
}

/** The families of protocol the form is shaped for: what is asked, and how a login is made. */
export type SchemeFamily = 'ssh' | 'smb' | 'dav' | 's3' | 'other';

export function familyOf(scheme: string): SchemeFamily {
	switch (scheme) {
		case 'sftp':
			return 'ssh';
		case 'smb':
			return 'smb';
		case 'dav':
		case 'davs':
			return 'dav';
		case 's3':
			return 's3';
		default:
			return 'other';
	}
}

/** The ways to sign in a protocol has, in the order they are offered. */
export function methodsFor(scheme: string): readonly AuthMethod[] {
	switch (familyOf(scheme)) {
		case 'ssh':
			return ['auto', 'password', 'keyFile'];
		case 'dav':
			return ['auto', 'password', 'token'];
		// An S3 login is an access key: the id and secret are their own fields, not a way to sign in.
		case 's3':
			return ['auto'];
		default:
			return ['auto', 'password'];
	}
}

/** The form after another protocol was chosen: a way to sign in the new one does not have goes back to automatic. */
export function withScheme(form: ConnectForm, scheme: string): ConnectForm {
	return {
		...form,
		scheme,
		auth: methodsFor(scheme).includes(form.auth) ? form.auth : 'auto',
	};
}

/** Splits an SMB user the way it is written (`domain;user`). */
function splitDomain(user: string): { domain: string; user: string } {
	const at = user.indexOf(';');
	return at < 0 ? { domain: '', user } : { domain: user.slice(0, at), user: user.slice(at + 1) };
}

/** The fields of a draft Rust read (a typed address) or a saved connection. */
export function formOf(draft: ConnectionDraft | SavedConnection, keep?: ConnectForm): ConnectForm {
	const login = familyOf(draft.scheme) === 'smb' ? splitDomain(draft.user ?? '') : null;
	const s3 =
		familyOf(draft.scheme) === 's3'
			? s3PresetOf(draft.options.s3Endpoint ?? null, draft.options.s3Preset)
			: null;
	return {
		name: draft.name || keep?.name || '',
		scheme: draft.scheme,
		host: draft.host,
		port: draft.port === null ? '' : String(draft.port),
		user: login ? login.user : (draft.user ?? ''),
		domain: login ? login.domain : '',
		// What a typed address cannot say (how to sign in, which dialect) stays as it was chosen.
		davAuth: keep && !('id' in draft) ? keep.davAuth : (draft.options.davAuth ?? 'auto'),
		nextcloud: keep && !('id' in draft) ? keep.nextcloud : draft.options.davPreset === 'nextcloud',
		auth: keep && !('id' in draft) ? keep.auth : draft.auth,
		keyFile: draft.keyFile ?? keep?.keyFile ?? '',
		jumpHost: draft.jumpHost ?? keep?.jumpHost ?? '',
		startFolder: draft.startFolder ?? '',
		thumbnails: draft.options.thumbnails,
		thumbnailMaxMb:
			draft.options.thumbnailMaxMb === null ? '' : String(draft.options.thumbnailMaxMb),
		refreshSeconds:
			draft.options.refreshSeconds === null ? '' : String(draft.options.refreshSeconds),
		s3Preset: s3 ? s3.preset : 'aws',
		s3Value: s3 ? s3.value : '',
		s3Region: draft.options.s3Region ?? (s3 && keep && !('id' in draft) ? keep.s3Region : ''),
		s3PathStyle:
			draft.options.s3PathStyle ?? (s3 && keep && !('id' in draft) ? keep.s3PathStyle : null),
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
	const family = familyOf(form.scheme);
	const user = blank(form.user);
	const domain = blank(form.domain);
	const s3 = family === 's3';
	const endpoint = s3 ? s3Endpoint(form.s3Preset, form.s3Value) : null;
	const thumbnailMax = count(form.thumbnailMaxMb);
	return {
		name: form.name.trim(),
		scheme: form.scheme,
		host: form.host.trim(),
		port: s3 ? null : port === null ? null : Number.isNaN(port) || port > 65535 ? 0 : port,
		// A domain goes with a user: `domain;user`. A domain alone is refused before it is sent.
		user: family === 'smb' && domain && user ? `${domain};${user}` : user,
		auth: methodsFor(form.scheme).includes(form.auth) ? form.auth : 'auto',
		keyFile: family === 'ssh' && form.auth === 'keyFile' ? blank(form.keyFile) : null,
		jumpHost: family === 'ssh' ? blank(form.jumpHost) : null,
		startFolder: blank(form.startFolder),
		options: {
			thumbnails: form.thumbnails,
			thumbnailMaxMb: thumbnailMax === null ? null : Number.isNaN(thumbnailMax) ? 0 : thumbnailMax,
			refreshSeconds: refresh === null ? null : Number.isNaN(refresh) ? 0 : refresh,
			timeoutSeconds: null,
			listingRequests: null,
			transferRequests: null,
			windowKib: null,
			davAuth: family === 'dav' && form.davAuth !== 'auto' ? form.davAuth : null,
			davPreset: family === 'dav' && form.nextcloud ? 'nextcloud' : null,
			s3Endpoint: endpoint ?? null,
			s3Region: s3 ? blank(form.s3Region) : null,
			s3PathStyle: s3 ? form.s3PathStyle : null,
			s3Preset: s3 ? form.s3Preset : null,
		},
	};
}

/** What is wrong with the form before it is sent, which Rust cannot see because the fields are joined first. */
export function formProblem(form: ConnectForm): { field: FormField; message: string } | null {
	if (familyOf(form.scheme) === 'smb' && form.domain.trim() !== '' && form.user.trim() === '') {
		return { field: 'user', message: t('connect.problem.domainUser') };
	}
	if (familyOf(form.scheme) === 's3') {
		if (form.host.trim() === '') return { field: 'host', message: t('connect.problem.s3Bucket') };
		if (s3Endpoint(form.s3Preset, form.s3Value) === undefined) {
			return { field: 's3Value', message: t(S3_VALUE_PROBLEM[s3Preset(form.s3Preset).input]) };
		}
	}
	return null;
}

const S3_VALUE_PROBLEM = {
	none: 'connect.problem.s3Endpoint',
	region: 'connect.problem.s3Region',
	accountId: 'connect.problem.s3Account',
	host: 'connect.problem.s3Endpoint',
	endpoint: 'connect.problem.s3Endpoint',
} as const satisfies Record<string, MessageId>;

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
	endpoint: 's3Value',
	region: 's3Region',
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
	endpoint: 'connect.problem.s3Endpoint',
	region: 'connect.problem.s3RegionName',
};

/** Where a refused save goes in the form, and what it says; `null` for something else. */
export function draftProblem(error: unknown): { field: FormField; message: string } | null {
	if (!isConnectionsRefusal(error)) return null;
	const refusal = error.error;
	switch (refusal.kind) {
		case 'draft':
			// The one option with a field of its own besides the refresh interval is the preview size cap.
			if (refusal.error.kind === 'option' && refusal.error.option === 'thumbnailMaxMb') {
				return { field: 'thumbnailMaxMb', message: t('connect.problem.thumbnailMaxMb') };
			}
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
		case 'clockSkew':
			return t('connect.error.clockSkew');
		case 'archived':
			return t('connect.error.archived');
		case 'unsupported':
			return tf('connect.error.unsupported', { what: error.what });
		case 'protocolOff':
			return tf('connect.error.protocolOff', { protocol: schemeLabel(error.scheme) });
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
