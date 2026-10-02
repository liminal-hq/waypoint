// The Services panel: every native plugin, whether it works here, what it can do and why not when it cannot
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { SettingsGroup } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsGroup';
import type { PluginStatus } from '@liminal-hq/waypoint-protocol/generated/PluginStatus';
import { useEffect, useState } from 'react';
import { t, tf, type MessageId } from '../i18n/messages';
import { collectServiceStatuses } from '../services/serviceStatuses';
import styles from './ServicesPanel.module.css';

interface ServicesPanelProps {
	/** Reads every plugin's status; the real plugins unless a test supplies its own. */
	load?: () => Promise<Record<string, PluginStatus>>;
}

/** The panel's name for a plugin; one without a message shows its key. */
function nameOf(key: string): string {
	const id = `services.name.${key}` as MessageId;
	return t(id) === id ? key : t(id);
}

function Service({ name, status }: { name: string; status: PluginStatus }) {
	const state = status.available
		? status.reason
			? t('services.state.partial')
			: t('services.state.available')
		: t('services.state.unavailable');
	return (
		<li
			className={styles.service}
			data-state={status.available ? (status.reason ? 'partial' : 'available') : 'unavailable'}
		>
			<div className={styles.head}>
				<span className={styles.name}>{name}</span>
				<span className={styles.state}>{state}</span>
			</div>
			{status.features.length > 0 && (
				<p className={styles.detail}>
					{tf('services.features', { features: status.features.join(', ') })}
				</p>
			)}
			{status.reason && <p className={styles.detail}>{status.reason}</p>}
		</li>
	);
}

/**
 * Settings → Integrations → Services. Every plugin the app registers is listed, available or not:
 * what works on this system and, for what does not, the reason (A66). The state is text as well as
 * a mark, so nothing depends on colour.
 */
export function ServicesPanel({ load = collectServiceStatuses }: ServicesPanelProps) {
	const [statuses, setStatuses] = useState<Record<string, PluginStatus> | null>(null);
	useEffect(() => {
		let live = true;
		load().then(
			(next) => live && setStatuses(next),
			() => live && setStatuses({}),
		);
		return () => {
			live = false;
		};
	}, [load]);
	return (
		<SettingsGroup title={t('services.title')}>
			<p className={styles.intro}>{t('services.intro')}</p>
			{statuses === null ? (
				<p role="status" className={styles.intro}>
					{t('services.loading')}
				</p>
			) : (
				<ul className={styles.list} aria-label={t('services.title')}>
					{Object.entries(statuses)
						.sort(([a], [b]) => nameOf(a).localeCompare(nameOf(b)))
						.map(([key, status]) => (
							<Service key={key} name={nameOf(key)} status={status} />
						))}
				</ul>
			)}
		</SettingsGroup>
	);
}
