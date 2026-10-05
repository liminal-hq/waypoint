// The Operations page: verifying copies, how many run at once, the undo history and the Trash sweep
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ButtonRow } from '@liminal-hq/waypoint-chrome/SettingsShell/ButtonRow';
import { NumberRow } from '@liminal-hq/waypoint-chrome/SettingsShell/NumberRow';
import { SelectRow } from '@liminal-hq/waypoint-chrome/SettingsShell/SelectRow';
import { SettingsGroup } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsGroup';
import { SettingsSection } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsSection';
import { ToggleRow } from '@liminal-hq/waypoint-chrome/SettingsShell/ToggleRow';
import { t, tf } from '../i18n/messages';
import {
	BYTES_PER_MB,
	DEFAULT_ARCHIVE_LIMITS,
	DEFAULT_OPS,
	DEFAULT_SPEED_LIMIT_MBPS,
	DEFAULT_TRASH_EXPIRY_DAYS,
} from './opsDefaults';
import { useSettingsEditor } from './SettingsEditor';

/** The number fields' ranges: narrower than what Rust accepts, for what a person would choose. */
export const CONCURRENCY_RANGE = { min: 1, max: 8 } as const;
export const SPEED_LIMIT_RANGE = { min: 1, max: 10_000 } as const;
export const UNDO_DEPTH_RANGE = { min: 1, max: 200 } as const;
export const TRASH_DAYS_RANGE = { min: 1, max: 36_500 } as const;

/**
 * The archive limits' ranges, which are the ranges Rust accepts (`ARCHIVE_*_RANGE` in the operations
 * plugin). The two sizes are shown in GiB and MiB and kept in bytes.
 */
export const ARCHIVE_ENTRIES_RANGE = { min: 1_000, max: 100_000_000 } as const;
export const ARCHIVE_GIB_RANGE = { min: 1, max: 1024 } as const;
export const ARCHIVE_RATIO_RANGE = { min: 10, max: 100_000 } as const;
export const ARCHIVE_MIB_RANGE = { min: 1, max: 1024 * 1024 } as const;
const MIB = 1024 ** 2;
const GIB = 1024 ** 3;

/** Whether every archive limit is already at its default, so there is nothing to reset. */
export function archiveLimitsAtDefaults(ops: typeof DEFAULT_OPS): boolean {
	return (
		ops.archiveMaxEntries === DEFAULT_ARCHIVE_LIMITS.archiveMaxEntries &&
		ops.archiveMaxBytes === DEFAULT_ARCHIVE_LIMITS.archiveMaxBytes &&
		ops.archiveMaxRatio === DEFAULT_ARCHIVE_LIMITS.archiveMaxRatio &&
		ops.archiveRatioFloorBytes === DEFAULT_ARCHIVE_LIMITS.archiveRatioFloorBytes
	);
}

export function OperationsPage() {
	const { ops, opsUnreadable, errors, changeOps } = useSettingsEditor();
	const current = ops ?? DEFAULT_OPS;
	const unreadable = ops === null;
	const sweep = current.trashExpiryDays !== null;
	const limited = current.speedLimitBps !== null;
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
				<ToggleRow
					label={t('settings.operations.speedLimit.label')}
					description={t('settings.operations.speedLimit.description')}
					error={errors.speedLimit}
					disabled={unreadable}
					checked={limited}
					onChange={(on) =>
						changeOps('speedLimit', (o) => ({
							...o,
							speedLimitBps: on
								? (o.speedLimitBps ?? DEFAULT_SPEED_LIMIT_MBPS * BYTES_PER_MB)
								: null,
						}))
					}
				/>
				<NumberRow
					label={t('settings.operations.speedLimitValue.label')}
					description={
						limited
							? t('settings.operations.speedLimitValue.description')
							: `${t('settings.operations.speedLimitValue.description')} ${t('settings.operations.speedLimitValue.needsLimit')}`
					}
					error={errors.speedLimitValue}
					disabled={unreadable || !limited}
					value={Math.round(
						(current.speedLimitBps ?? DEFAULT_SPEED_LIMIT_MBPS * BYTES_PER_MB) / BYTES_PER_MB,
					)}
					{...SPEED_LIMIT_RANGE}
					step={1}
					unit={t('settings.operations.speedLimitValue.unit')}
					commitOn="commit"
					onChange={(mb) =>
						changeOps('speedLimitValue', (o) => ({ ...o, speedLimitBps: mb * BYTES_PER_MB }))
					}
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
			<SettingsGroup title={t('settings.group.archives')}>
				<NumberRow
					label={t('settings.operations.archiveEntries.label')}
					description={t('settings.operations.archiveEntries.description')}
					error={errors.archiveEntries}
					disabled={unreadable}
					value={current.archiveMaxEntries}
					{...ARCHIVE_ENTRIES_RANGE}
					step={1000}
					unit={t('settings.operations.archiveEntries.unit')}
					commitOn="commit"
					onChange={(archiveMaxEntries) =>
						changeOps('archiveEntries', (o) => ({ ...o, archiveMaxEntries }))
					}
				/>
				<NumberRow
					label={t('settings.operations.archiveBytes.label')}
					description={t('settings.operations.archiveBytes.description')}
					error={errors.archiveBytes}
					disabled={unreadable}
					value={Math.round(current.archiveMaxBytes / GIB)}
					{...ARCHIVE_GIB_RANGE}
					step={1}
					unit={t('settings.operations.archiveBytes.unit')}
					commitOn="commit"
					onChange={(gib) =>
						changeOps('archiveBytes', (o) => ({ ...o, archiveMaxBytes: gib * GIB }))
					}
				/>
				<NumberRow
					label={t('settings.operations.archiveRatio.label')}
					description={t('settings.operations.archiveRatio.description')}
					error={errors.archiveRatio}
					disabled={unreadable}
					value={current.archiveMaxRatio}
					{...ARCHIVE_RATIO_RANGE}
					step={10}
					unit={t('settings.operations.archiveRatio.unit')}
					commitOn="commit"
					onChange={(archiveMaxRatio) =>
						changeOps('archiveRatio', (o) => ({ ...o, archiveMaxRatio }))
					}
				/>
				<NumberRow
					label={t('settings.operations.archiveFloor.label')}
					description={t('settings.operations.archiveFloor.description')}
					error={errors.archiveFloor}
					disabled={unreadable}
					value={Math.round(current.archiveRatioFloorBytes / MIB)}
					{...ARCHIVE_MIB_RANGE}
					step={1}
					unit={t('settings.operations.archiveFloor.unit')}
					commitOn="commit"
					onChange={(mib) =>
						changeOps('archiveFloor', (o) => ({ ...o, archiveRatioFloorBytes: mib * MIB }))
					}
				/>
				<ButtonRow
					label={t('settings.operations.archiveReset.label')}
					description={t('settings.operations.archiveReset.description')}
					error={errors.archiveReset}
					actionLabel={t('settings.operations.archiveReset.action')}
					disabled={unreadable || archiveLimitsAtDefaults(current)}
					onAction={() => changeOps('archiveReset', (o) => ({ ...o, ...DEFAULT_ARCHIVE_LIMITS }))}
				/>
			</SettingsGroup>
		</SettingsSection>
	);
}
