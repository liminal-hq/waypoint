// The Operations page: verifying copies, how many run at once, the undo history and the Trash sweep
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { NumberRow } from '@liminal-hq/waypoint-chrome/SettingsShell/NumberRow';
import { SelectRow } from '@liminal-hq/waypoint-chrome/SettingsShell/SelectRow';
import { SettingsGroup } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsGroup';
import { SettingsSection } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsSection';
import { ToggleRow } from '@liminal-hq/waypoint-chrome/SettingsShell/ToggleRow';
import { t, tf } from '../i18n/messages';
import { DEFAULT_OPS, DEFAULT_TRASH_EXPIRY_DAYS } from './opsDefaults';
import { useSettingsEditor } from './SettingsEditor';

/** The number fields' ranges: narrower than what Rust accepts, for what a person would choose. */
export const CONCURRENCY_RANGE = { min: 1, max: 8 } as const;
export const UNDO_DEPTH_RANGE = { min: 1, max: 200 } as const;
export const TRASH_DAYS_RANGE = { min: 1, max: 36_500 } as const;

export function OperationsPage() {
	const { ops, opsUnreadable, errors, changeOps } = useSettingsEditor();
	const current = ops ?? DEFAULT_OPS;
	const unreadable = ops === null;
	const sweep = current.trashExpiryDays !== null;
	return (
		<SettingsSection
			description={
				opsUnreadable ? tf('settings.ops.unreadable', { reason: opsUnreadable }) : undefined
			}
		>
			<SettingsGroup title={t('settings.group.copying')}>
				<ToggleRow
					label={t('settings.operations.verify.label')}
					description={t('settings.operations.verify.description')}
					error={errors.verify}
					disabled={unreadable}
					checked={current.verifyAfterCopy}
					onChange={(verifyAfterCopy) => changeOps('verify', (o) => ({ ...o, verifyAfterCopy }))}
				/>
				<SelectRow
					label={t('settings.operations.algorithm.label')}
					description={
						current.verifyAfterCopy
							? t('settings.operations.algorithm.description')
							: `${t('settings.operations.algorithm.description')} ${t('settings.operations.algorithm.needsVerify')}`
					}
					error={errors.algorithm}
					disabled={unreadable || !current.verifyAfterCopy}
					value={current.verifyAlgorithm}
					options={[
						{ value: 'blake3', label: t('settings.operations.algorithm.blake3') },
						{ value: 'sha256', label: t('settings.operations.algorithm.sha256') },
					]}
					onChange={(verifyAlgorithm) => changeOps('algorithm', (o) => ({ ...o, verifyAlgorithm }))}
				/>
			</SettingsGroup>
			<SettingsGroup title={t('settings.group.queue')}>
				<NumberRow
					label={t('settings.operations.concurrency.label')}
					description={t('settings.operations.concurrency.description')}
					error={errors.concurrency}
					disabled={unreadable}
					value={current.concurrency}
					{...CONCURRENCY_RANGE}
					step={1}
					commitOn="commit"
					onChange={(concurrency) => changeOps('concurrency', (o) => ({ ...o, concurrency }))}
				/>
				<NumberRow
					label={t('settings.operations.undoDepth.label')}
					description={t('settings.operations.undoDepth.description')}
					error={errors.undoDepth}
					disabled={unreadable}
					value={current.undoDepth}
					{...UNDO_DEPTH_RANGE}
					step={1}
					commitOn="commit"
					onChange={(undoDepth) => changeOps('undoDepth', (o) => ({ ...o, undoDepth }))}
				/>
			</SettingsGroup>
			<SettingsGroup title={t('settings.group.trash')}>
				<ToggleRow
					label={t('settings.operations.trashExpiry.label')}
					description={t('settings.operations.trashExpiry.description')}
					error={errors.trashExpiry}
					disabled={unreadable}
					checked={sweep}
					onChange={(on) =>
						changeOps('trashExpiry', (o) => ({
							...o,
							trashExpiryDays: on ? (o.trashExpiryDays ?? DEFAULT_TRASH_EXPIRY_DAYS) : null,
						}))
					}
				/>
				<NumberRow
					label={t('settings.operations.trashDays.label')}
					description={
						sweep
							? t('settings.operations.trashDays.description')
							: `${t('settings.operations.trashDays.description')} ${t('settings.operations.trashDays.needsExpiry')}`
					}
					error={errors.trashDays}
					disabled={unreadable || !sweep}
					value={current.trashExpiryDays ?? DEFAULT_TRASH_EXPIRY_DAYS}
					{...TRASH_DAYS_RANGE}
					step={1}
					unit={t('settings.operations.trashDays.unit')}
					commitOn="commit"
					onChange={(trashExpiryDays) => changeOps('trashDays', (o) => ({ ...o, trashExpiryDays }))}
				/>
			</SettingsGroup>
		</SettingsSection>
	);
}
