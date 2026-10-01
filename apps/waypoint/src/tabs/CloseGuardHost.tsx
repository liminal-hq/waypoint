// Gives the tab actions below it the D29 close guard, asking in the chrome's confirm dialog
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useMemo, type ReactNode } from 'react';
import { t } from '../i18n/messages';
import { useConfirm } from '../ops/ConfirmHost';
import { useOps } from '../ops/OpsContext';
import { CloseGuardProvider, createCloseGuard } from './closeGuard';

/**
 * Closing a half of a pair while an operation writes to the other half's folder asks first
 * ("An operation is still writing to this folder"). Without a queue (a window made without the
 * plugin) nothing is guarded. Must sit inside `MainOps`.
 */
export function CloseGuardHost({ children }: { children: ReactNode }) {
	const ops = useOps();
	const { confirm, dialog } = useConfirm();
	const handle = ops?.handle;
	const guard = useMemo(
		() =>
			handle
				? createCloseGuard(
						(location) => handle.jobsTargeting(location),
						() =>
							confirm({
								title: t('tabs.closeGuard.title'),
								message: t('tabs.closeGuard.message'),
								confirmLabel: t('tabs.closeGuard.action'),
								cancelLabel: t('tabs.closeGuard.keep'),
								// Focus starts on Keep Open: a close that cuts a view off a running job is not the default.
								danger: true,
							}),
					)
				: undefined,
		[handle, confirm],
	);
	return (
		<>
			{guard ? <CloseGuardProvider value={guard}>{children}</CloseGuardProvider> : children}
			{dialog}
		</>
	);
}
