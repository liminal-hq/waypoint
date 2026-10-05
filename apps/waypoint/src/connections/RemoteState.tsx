// A folder on a server that cannot be shown: why, in words, with Reconnect, Sign In or Review; and the connecting state
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import { createContext, useContext, useState, type ReactNode } from 'react';
import { useVfsClient } from '../browse/VfsClientContext';
import { t, tf } from '../i18n/messages';
import { useLocationInfo } from '../nav/locationInfo';
import { announce } from '../tabs/announcer';
import { connectAnswering } from './connectFlow';
import { connectionErrorText } from './connectModel';
import { useConnections, useConnectionsView } from './ConnectionsContext';
import { stateOf } from './connectionsModel';
import { askQuestion } from './connectStore';
import { ReconnectIcon } from './ConnectionIcons';
import { remoteStateText } from './remoteModel';
import styles from './RemoteState.module.css';

/** Opens again the folders on screen whose connection failed; the Workspace supplies it. */
const RetryContext = createContext<() => void>(() => {});

export function RetryProvider({ retry, children }: { retry: () => void; children: ReactNode }) {
	return <RetryContext.Provider value={retry}>{children}</RetryContext.Provider>;
}

/** The server a location is on, as the breadcrumbs name it (its saved name, or its login). */
function useServerName(location: Location): string {
	const info = useLocationInfo(useVfsClient(), location);
	return info?.segments[0]?.label ?? location.display;
}

/**
 * The state of a folder whose server could not be reached or asked something (SPEC §5.5, D146):
 * a title and a reason in words, and one action. Reconnect tries again; Sign In and Review ask the
 * question first (the window's question dialogs). Once it connects, every folder on screen that
 * failed for its connection opens again.
 */
export function RemoteState({ error }: { error: VfsError }) {
	const connections = useConnections();
	const retry = useContext(RetryContext);
	const location = 'location' in error && error.location ? error.location : null;
	const server = useServerName(location ?? { display: '', uri: '' });
	const [busy, setBusy] = useState(false);
	const [problem, setProblem] = useState<string | null>(null);
	const text = remoteStateText(error);

	const act = async () => {
		if (!connections || !location || busy) return;
		setBusy(true);
		setProblem(null);
		const first = text.action === 'reconnect' ? null : await askQuestion(error);
		if (text.action !== 'reconnect' && first === null) {
			setBusy(false);
			return;
		}
		const outcome = await connectAnswering(
			(answer, remember) => connections.client.connect(location, answer, remember),
			askQuestion,
			first,
		);
		setBusy(false);
		if (outcome.kind === 'connected') {
			announce(tf('remote.announce.connected', { server }));
			retry();
		} else if (outcome.kind === 'failed') {
			const words = connectionErrorText(outcome.error);
			setProblem(words);
			announce(words);
		}
	};

	const label =
		text.action === 'signIn'
			? t('remote.action.signIn')
			: text.action === 'review'
				? t('remote.action.review')
				: t('remote.action.reconnect');
	return (
		<div className={styles.state} role="alert" data-error={error.kind}>
			<h2 className={styles.title}>{tf(text.title, { server })}</h2>
			<p className={styles.detail}>{tf(text.detail, { server })}</p>
			{connections && location && (
				<button type="button" className={styles.action} disabled={busy} onClick={() => void act()}>
					<ReconnectIcon className={styles.icon} />
					{busy ? t('remote.action.connecting') : label}
				</button>
			)}
			{problem && <p className={styles.problem}>{problem}</p>}
		</div>
	);
}

/**
 * While a server folder opens and its login is connecting, the view says so (with the server's
 * name) instead of the plain opening message.
 */
export function RemoteOpening({ location, fallback }: { location: Location; fallback: ReactNode }) {
	const info = useLocationInfo(useVfsClient(), location);
	const key = info?.connection;
	const connecting = useConnectionsView((view) =>
		key ? stateOf(view, key).kind === 'connecting' : false,
	);
	if (!key || !connecting) return <>{fallback}</>;
	return (
		<div className={styles.state} role="status" data-state="connecting">
			<span className={styles.spinner} aria-hidden="true" />
			<p className={styles.detail}>
				{tf('remote.connecting', { server: info?.segments[0]?.label ?? location.display })}
			</p>
		</div>
	);
}
