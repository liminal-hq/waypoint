// Gives a Main window its operations queue: the ring's store, the toasts, the announcements and the recovery notice
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useMemo, type ReactNode } from 'react';
import { showNotice } from '../app/notices';
import { t } from '../i18n/messages';
import type { Location, OpsClient } from '../services/opsClient';
import { announce } from '../tabs/announcer';
import { useActiveTab, useTabsApi } from '../tabs/TabsContext';
import { OpsProvider } from './OpsContext';
import { OpsResolverHost } from './OpsResolverHost';
import { startOpsAnnouncer } from './opsAnnouncer';
import { showRecoveryNotice, startUndoNotices } from './opsNotices';
import type { OpsHandle } from './opsStore';
import { currentWindowLabel } from './windowLabel';

interface MainOpsProps {
	/** Without a client (a window made without the plugin) the ring is simply absent. */
	client?: OpsClient;
	children: ReactNode;
}

/** Opens the single Operations window, or brings it forward. */
export function openOpsWindow(): void {
	invoke<void>('open_ops_window').catch((error: unknown) => {
		console.warn('could not open the Operations window', error);
		showNotice(t('ops.popOut.failed'));
	});
}

/** Must sit inside the `TabsProvider`: a finished job's folder is shown in the active tab. */
export function MainOps({ client, children }: MainOpsProps) {
	const windowLabel = useMemo(currentWindowLabel, []);
	const api = useTabsApi();
	const tab = useActiveTab();
	const tabId = tab?.id;
	const showInFolder = useCallback(
		(location: Location) => {
			if (tabId !== undefined) void api.navigate(tabId, location);
		},
		[api, tabId],
	);
	const onHandle = useCallback(
		(handle: OpsHandle) => {
			const stopAnnouncer = startOpsAnnouncer(handle, {
				announce,
				include: (job) => job.originWindow === windowLabel,
			});
			const stopNotices = startUndoNotices(handle, { windowLabel });
			return () => {
				stopAnnouncer();
				stopNotices();
			};
		},
		[windowLabel],
	);
	// Asked once per window start; Rust hands the report to the first window that asks.
	useEffect(() => {
		if (client) void showRecoveryNotice(client);
	}, [client]);

	if (!client) return <>{children}</>;
	return (
		<OpsProvider
			client={client}
			windowLabel={windowLabel}
			popOut={openOpsWindow}
			showInFolder={showInFolder}
			onHandle={onHandle}
		>
			{children}
			<OpsResolverHost />
		</OpsProvider>
	);
}
