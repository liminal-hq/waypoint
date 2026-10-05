// What a protocol that is turned off says, and the link to the page that turns it on
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t, tf } from '../i18n/messages';
import { openSettingsWindow } from '../settings/openSettingsWindow';
import { schemeLabel } from './connectModel';
import styles from './ProtocolOff.module.css';
import state from './RemoteState.module.css';

/** The id of the Settings page the remote protocols' switches are on (D167). */
export const EXPERIMENTAL_SECTION = 'experimental';

/** A link-styled button that opens Settings → Experimental, in the one Settings window. */
export function ExperimentalLink({
	open = openSettingsWindow,
}: {
	open?: (section: string) => void;
}) {
	return (
		<button type="button" className={styles.link} onClick={() => open(EXPERIMENTAL_SECTION)}>
			{t('protocol.off.action')}
		</button>
	);
}

/**
 * Where a folder would be, for an address whose protocol is turned off: says which protocol and
 * where to turn it on, with the link (D167). Never a blank view, never a crash.
 */
export function ProtocolOffState({ scheme }: { scheme: string }) {
	return (
		<div className={state.state} role="alert" data-error="protocolOff">
			<h2 className={state.title}>{tf('protocol.off.title', { protocol: schemeLabel(scheme) })}</h2>
			<p className={state.detail}>{t('protocol.off.detail')}</p>
			<ExperimentalLink />
		</div>
	);
}
