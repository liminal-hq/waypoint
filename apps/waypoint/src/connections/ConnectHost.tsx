// Shows the Connect dialog and a connection's questions in a Main window, and opens what connects in a new tab
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { KeyringUnavailable } from '@liminal-hq/waypoint-protocol/generated/KeyringUnavailable';
import { useEffect, useState } from 'react';
import { useStore } from 'zustand';
import { useCommandBridge } from '../commands/commandBridge';
import { announce } from '../tabs/announcer';
import { useTabsApi } from '../tabs/TabsContext';
import { ConnectDialog } from './ConnectDialog';
import { useConnections, useConnectionsView } from './ConnectionsContext';
import { askQuestion, connectStore, openConnectDialog } from './connectStore';
import { QuestionDialog } from './QuestionDialogs';

/**
 * Mounted once per Main window. Connect to Server… (the menu, the palette, the + button and the
 * Network section) opens the dialog through `openConnectDialog`; a question a connection asks
 * anywhere in the window (the dialog, a tab's Sign in) shows here through `askQuestion`. Without a
 * connections client the window offers neither.
 */
export function ConnectHost() {
	const connections = useConnections();
	const bridge = useCommandBridge();
	const dialog = useStore(connectStore, (state) => state.dialog);
	const question = useStore(connectStore, (state) => state.question);
	const saved = useConnectionsView((view) => view.connections);
	const api = useTabsApi();
	const [keyring, setKeyring] = useState<KeyringUnavailable | null>('noKeyring');

	useEffect(() => {
		bridge.patchFacts({ connections: connections !== null });
		bridge.patchActions({ connectToServer: () => openConnectDialog() });
	}, [bridge, connections]);

	// Whether a login can be remembered is asked again for each question: the keyring may have been
	// unlocked or started since.
	useEffect(() => {
		if (!question || !connections) return;
		let live = true;
		connections.client.support().then(
			(support) => live && setKeyring(support.keyring),
			() => live && setKeyring('noKeyring'),
		);
		return () => {
			live = false;
		};
	}, [question, connections]);

	if (!connections) return null;
	return (
		<>
			{dialog && (
				<ConnectDialog
					client={connections.client}
					request={dialog}
					saved={saved}
					ask={askQuestion}
					announce={announce}
					onOpen={(location) => {
						void api.openTab(location).catch((error: unknown) => {
							console.warn('could not open the server in a new tab', error);
						});
					}}
					onClose={() => connectStore.getState().close()}
				/>
			)}
			{question && (
				<QuestionDialog
					error={question.error}
					keyring={keyring}
					onAnswer={(answered) => connectStore.getState().answer(answered)}
				/>
			)}
		</>
	);
}
