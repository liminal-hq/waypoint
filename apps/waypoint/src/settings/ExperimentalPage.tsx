// The Experimental page: one switch per remote protocol, each off until it is turned on
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { SettingsGroup } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsGroup';
import { SettingsSection } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsSection';
import { ToggleRow } from '@liminal-hq/waypoint-chrome/SettingsShell/ToggleRow';
import { t, type MessageId } from '../i18n/messages';
import type { Settings } from '../services/settingsClient';
import styles from './ExperimentalPage.module.css';
import { useSettingsEditor, type RowKey } from './SettingsEditor';

type Switch = keyof Settings['experimental'];

interface ProtocolRow {
	switch: Switch;
	key: RowKey;
	/** The URI schemes the switch gates, to tell whether this build has the provider. */
	schemes: readonly string[];
	label: MessageId;
	description: MessageId;
}

/** The protocols in the order the page lists them. */
const ROWS: readonly ProtocolRow[] = [
	{
		switch: 'sftp',
		key: 'protocolSftp',
		schemes: ['sftp'],
		label: 'settings.experimental.sftp.label',
		description: 'settings.experimental.sftp.description',
	},
	{
		switch: 'smb',
		key: 'protocolSmb',
		schemes: ['smb'],
		label: 'settings.experimental.smb.label',
		description: 'settings.experimental.smb.description',
	},
	{
		switch: 'webdav',
		key: 'protocolWebdav',
		schemes: ['dav', 'davs'],
		label: 'settings.experimental.webdav.label',
		description: 'settings.experimental.webdav.description',
	},
	{
		switch: 's3',
		key: 'protocolS3',
		schemes: ['s3'],
		label: 'settings.experimental.s3.label',
		description: 'settings.experimental.s3.description',
	},
];

/**
 * Settings → Experimental. Each remote protocol has a switch, off until the person turns it on
 * (D167); a change registers or turns off its provider at once, with no restart. A protocol this
 * build does not include has its switch dimmed with the reason, so the page lists what will exist.
 * The badge is part of each row's label, so a screen reader hears "SFTP Experimental" for the switch.
 */
export function ExperimentalPage() {
	const { settings, errors, changeSettings, protocols } = useSettingsEditor();
	const included = (row: ProtocolRow): boolean =>
		// Until the build's protocols are read (or when they cannot be) a switch stays usable.
		protocols === null ||
		row.schemes.some(
			(scheme) => protocols.schemes.includes(scheme) || protocols.off.includes(scheme),
		);
	return (
		<SettingsSection description={t('settings.experimental.intro')}>
			<SettingsGroup title={t('settings.group.protocols')}>
				{ROWS.map((row) => (
					<ToggleRow
						key={row.switch}
						label={
							<>
								{t(row.label)}
								<span className={styles.badge}>{t('settings.experimental.badge')}</span>
							</>
						}
						description={t(row.description)}
						unavailableReason={included(row) ? undefined : t('settings.experimental.unavailable')}
						error={errors[row.key]}
						checked={settings.experimental[row.switch]}
						onChange={(value) =>
							changeSettings(row.key, (s) => ({
								...s,
								experimental: { ...s.experimental, [row.switch]: value },
							}))
						}
					/>
				))}
			</SettingsGroup>
		</SettingsSection>
	);
}
