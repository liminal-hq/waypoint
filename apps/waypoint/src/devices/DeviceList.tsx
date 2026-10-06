// The Devices section's list: each volume with its usage, and the actions it allows
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { PluginStatus, Volume } from '@liminal-hq/plugin-volumes';
import { t, tf, type MessageId } from '../i18n/messages';
import { DriveIcon } from '../icons/MenuIcons';
import { ITEM_ATTRIBUTE, moveFocusInList } from '../sidebar/itemList';
import sidebar from '../sidebar/Sidebar.module.css';
import { actionsFor, statusText, usageOf, type DeviceAction } from './deviceModel';
import { EjectIcon, ForgetIcon, LockIcon, MountIcon, UnmountIcon } from './DeviceIcons';
import styles from './DeviceList.module.css';

const ACTION_LABELS: Record<DeviceAction, MessageId> = {
	mount: 'devices.action.mount',
	unmount: 'devices.action.unmount',
	eject: 'devices.action.eject',
	unlock: 'devices.action.unlock',
	forget: 'devices.action.forget',
};

const ACTION_ICONS = {
	mount: MountIcon,
	unmount: UnmountIcon,
	eject: EjectIcon,
	unlock: LockIcon,
	forget: ForgetIcon,
};

interface DeviceListProps {
	volumes: readonly Volume[];
	status: PluginStatus | null;
	busy: ReadonlySet<string>;
	/** Opens a mounted volume; on one that is not mounted it does what the row's first action does. */
	onActivate(volume: Volume): void;
	onAction(action: DeviceAction, volume: Volume): void;
}

/**
 * One row per volume. The row's button opens a mounted volume, mounts one that is not mounted and
 * asks for the passphrase of a locked one; Up, Down, Home and End move between rows like the other
 * lists. The actions follow as buttons of their own, only those the volume allows and the plugin
 * reports as working here. The space is written under the name as well as drawn, so no colour is
 * needed to read it; a nearly full volume says so and its bar is hatched.
 */
export function DeviceList({ volumes, status, busy, onActivate, onAction }: DeviceListProps) {
	if (volumes.length === 0) return <p className={sidebar.empty}>{t('devices.empty')}</p>;
	return (
		<ul className={sidebar.list} aria-label={t('devices.list')} onKeyDown={moveFocusInList}>
			{volumes.map((volume) => {
				const working = busy.has(volume.id);
				const usage = usageOf(volume);
				const actions = actionsFor(volume, status);
				// The warning gets a line of its own, so a narrow sidebar or a long translation wraps it
				// rather than cutting it off. Locked and busy rows have no space line to warn on.
				const warning = !working && !volume.locked ? (usage?.warning ?? null) : null;
				const detail = working
					? t('devices.busy')
					: warning && usage
						? usage.summary
						: statusText(volume);
				const description = [volume.label, working ? t('devices.busy') : statusText(volume)].join(
					'. ',
				);
				return (
					<li key={volume.id} className={styles.row} aria-busy={working || undefined}>
						<button
							type="button"
							{...{ [ITEM_ATTRIBUTE]: '' }}
							className={`${sidebar.item} ${styles.item}`}
							disabled={working}
							data-unmounted={volume.mountPoint === null ? '' : undefined}
							title={[description, volume.device ?? volume.mountPoint].filter(Boolean).join('\n')}
							onClick={() => onActivate(volume)}
						>
							<DriveIcon className={sidebar.itemIcon} />
							<span className={styles.text}>
								<span className={sidebar.label}>{volume.label}</span>
								<span className={styles.detail}>{detail}</span>
								{warning && (
									<>
										{' '}
										<span className={styles.warning}>{warning}</span>
									</>
								)}
								{usage && (
									<span
										className={styles.bar}
										aria-hidden="true"
										data-full={usage.almostFull ? '' : undefined}
									>
										<span className={styles.fill} style={{ inlineSize: `${usage.percent}%` }} />
									</span>
								)}
							</span>
						</button>
						{actions.map((action) => {
							const Icon = ACTION_ICONS[action];
							const label = tf(ACTION_LABELS[action], { name: volume.label });
							return (
								<button
									key={action}
									type="button"
									className={styles.action}
									aria-label={label}
									title={label}
									disabled={working}
									onClick={() => onAction(action, volume)}
								>
									<Icon />
								</button>
							);
						})}
					</li>
				);
			})}
		</ul>
	);
}
