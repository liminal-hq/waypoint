// The states every file view shares: opening, failed, and the messages a folder can show
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import type { ReactNode } from 'react';
import { t, tf, type MessageId } from '../i18n/messages';
import { ProtocolOffState } from '../connections/ProtocolOff';
import { RemoteState } from '../connections/RemoteState';
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
		default:
			return { title: 'browse.error.other.title', detail: 'browse.error.other.detail' };
	}
}

/** The distinct state for a folder that could not be shown: never a blank view. A folder on a server that could not be reached, or that asks to sign in, has a state of its own with its action. */
export function ErrorState({ error }: { error: VfsError }) {
	if (error.kind === 'protocolOff') return <ProtocolOffState scheme={error.scheme} />;
	if (isConnectionError(error)) return <RemoteState error={error} />;
	const { title, detail } = errorMessages(error);
	const location =
		error.kind === 'notFound' || error.kind === 'permissionDenied' || error.kind === 'notADirectory'
			? error.location.display
			: '';
	return (
		<div className={styles.message} role="alert" data-error={error.kind}>
			<h2 className={styles.messageTitle}>{t(title)}</h2>
			<p className={styles.messageDetail}>{tf(detail, { location })}</p>
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
