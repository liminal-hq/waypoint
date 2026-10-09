// The states every file view shares: opening, failed, and the messages a folder can show
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { ReactNode } from 'react';
import { useStore } from 'zustand';
import { useCommandBridge } from '../commands/commandBridge';
import { isElevatedLocation, isFileUri } from '../elevation/elevatedLocation';
import { ShieldIcon } from '../icons/AppIcons';
import { t, tf, type MessageId } from '../i18n/messages';
import { ProtocolOffState } from '../connections/ProtocolOff';
import { ArchiveLocked } from '../archives/ArchiveLocked';
import { isArchiveLock } from '../archives/lockModel';
import { AdministratorState, RemoteState } from '../connections/RemoteState';
import { isConnectionError } from '../connections/remoteModel';
import styles from './ListingGate.module.css';
import type { ListingSession, SessionState } from './useListingSession';

function errorMessages(error: VfsError): { title: MessageId; detail: MessageId } {
	switch (error.kind) {
		case 'notFound':
			return { title: 'browse.error.notFound.title', detail: 'browse.error.notFound.detail' };
		case 'permissionDenied':
			return {
				title: 'browse.error.permissionDenied.title',
				detail: 'browse.error.permissionDenied.detail',
			};
		case 'notADirectory':
			return {
				title: 'browse.error.notADirectory.title',
				detail: 'browse.error.notADirectory.detail',
			};
		case 'corrupt':
			return { title: 'browse.error.corrupt.title', detail: 'browse.error.corrupt.detail' };
		case 'unsupported':
			return {
				title: 'browse.error.unsupported.title',
				detail: 'browse.error.unsupported.detail',
			};
		case 'archived':
			return {
				title: 'browse.error.archived.title',
				detail: 'browse.error.archived.detail',
			};
		default:
			return { title: 'browse.error.other.title', detail: 'browse.error.other.detail' };
	}
}

/** The distinct state for a folder that could not be shown: never a blank view. A folder on a server that could not be reached, or that asks to sign in, has a state of its own with its action. */
export function ErrorState({ error }: { error: VfsError }) {
	if (error.kind === 'protocolOff') return <ProtocolOffState scheme={error.scheme} />;
	// A password asked for an archive is the archive's, even when it is on a server.
	if (isArchiveLock(error)) return <ArchiveLocked error={error} />;
	if (isConnectionError(error)) {
		// A folder shown as an administrator that is not connected asks for approval, not a sign-in.
		const elevated = 'location' in error && isElevatedLocation(error.location);
		return elevated ? <AdministratorState error={error} /> : <RemoteState error={error} />;
	}
	const { title, detail } = errorMessages(error);
	const location =
		error.kind === 'notFound' ||
		error.kind === 'permissionDenied' ||
		error.kind === 'notADirectory' ||
		error.kind === 'corrupt' ||
		error.kind === 'archived'
			? error.location.display
			: '';
	const what = error.kind === 'unsupported' ? error.what : '';
	return (
		<div className={styles.message} role="alert" data-error={error.kind}>
			<h2 className={styles.messageTitle}>{t(title)}</h2>
			<p className={styles.messageDetail}>{tf(detail, { location, what })}</p>
			{error.kind === 'permissionDenied' && <OpenAsAdministrator location={error.location} />}
		</div>
	);
}

/**
 * The primary action of a folder that could not be opened for lack of permission: show it as an
 * administrator, when that is on and works here and the folder is an ordinary local one.
 */
function OpenAsAdministrator({ location }: { location: Location }) {
	const bridge = useCommandBridge();
	const offered = useStore(bridge.store, (env) => env.facts.elevation);
	if (!offered || !isFileUri(location.uri)) return null;
	return (
		<div className={styles.messageActions}>
			<button
				type="button"
				className={styles.primary}
				onClick={() => bridge.store.getState().actions.openAsAdministrator(location)}
			>
				<ShieldIcon width={16} height={16} />
				{t('cmd.openAsAdministrator')}
			</button>
		</div>
	);
}

interface MessageStateProps {
	role: 'status' | 'alert';
	'data-state'?: string;
	children: ReactNode;
}

/** A centred one-line message, such as the empty folder. */
export function MessageState({ role, children, ...rest }: MessageStateProps) {
	return (
		<div className={styles.message} role={role} {...rest}>
			{children}
		</div>
	);
}

interface ListingGateProps {
	state: SessionState;
	children: (session: ListingSession) => ReactNode;
}

/** Shows the opening and failed states, and hands a ready listing to `children`. */
export function ListingGate({ state, children }: ListingGateProps) {
	if (state.status === 'opening')
		return <MessageState role="status">{t('browse.opening')}</MessageState>;
	if (state.status === 'error') return <ErrorState error={state.error} />;
	return <>{children(state.session)}</>;
}
