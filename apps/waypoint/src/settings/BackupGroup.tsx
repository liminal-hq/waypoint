// The General page's Back up and restore group: save the settings to a file, load them back after a plan
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ButtonRow } from '@liminal-hq/waypoint-chrome/SettingsShell/ButtonRow';
import { SettingsGroup } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsGroup';
import { Dialog } from '@liminal-hq/waypoint-chrome/Dialog/Dialog';
import { DialogActions, DialogButton } from '@liminal-hq/waypoint-chrome/Dialog/DialogActions';
import { useState } from 'react';
import type { ImportPlan } from '@liminal-hq/waypoint-protocol/generated/ImportPlan';
import { showNotice } from '../app/notices';
import { t, tf, tn, type MessageId } from '../i18n/messages';
import { isSettingsError } from '../services/settingsClient';
import type { ImportPreview } from '../services/settingsTransferClient';
import { useSettingsEditor } from './SettingsEditor';
import styles from './BackupGroup.module.css';

/** The settings pages' own names for the groups of a settings file, for the list of what would change. */
const GROUP_LABELS: Record<string, MessageId> = {
	general: 'settings.section.general',
	ops: 'settings.section.operations',
	dnd: 'settings.section.dnd',
	appearance: 'settings.section.appearance',
	accessibility: 'settings.section.accessibility',
	integrations: 'settings.section.integrations',
	previews: 'settings.section.previews',
	transparency: 'settings.section.transparency',
	locale: 'settings.section.language',
	ui: 'settings.backup.group.ui',
	folders: 'settings.backup.group.folders',
};

const REASON_MESSAGES: Record<string, MessageId> = {
	'not-a-bundle': 'settings.backup.error.notABundle',
	corrupt: 'settings.backup.error.corrupt',
	'newer-format': 'settings.backup.error.newerFormat',
	'too-large': 'settings.backup.error.tooLarge',
	unsafe: 'settings.backup.error.unsafe',
	nothing: 'settings.backup.error.nothing',
};

/** The sentence under a row for an export or import that could not be done, in the person's language. */
export function transferErrorMessage(error: unknown): string {
	if (!isSettingsError(error)) {
		return tf('settings.backup.error.io', {
			detail: error instanceof Error ? error.message : t('settings.error.generic'),
		});
	}
	switch (error.kind) {
		case 'transfer': {
			if (error.reason === 'invalid') {
				return tf('settings.backup.error.invalid', { detail: error.message });
			}
			const known = error.reason ? REASON_MESSAGES[error.reason] : undefined;
			return known ? t(known) : tf('settings.backup.error.io', { detail: error.message });
		}
		case 'unavailable':
			return t('settings.backup.error.unavailable');
		case 'stale':
			return t('settings.backup.error.stale');
		case 'apply':
			return tf('settings.backup.error.apply', { detail: error.message });
		default:
			return tf('settings.backup.error.io', { detail: error.message });
	}
}

/** One line per kind of warning, with how many it is about. */
function warningLines(plan: ImportPlan): string[] {
	return plan.warnings.map((warning) => {
		switch (warning.kind) {
			case 'unknownKeys':
				return tn('settings.backup.warning.unknownKeys', warning.keys.length);
			case 'unknownFiles':
				return tn('settings.backup.warning.unknownFiles', warning.ids.length);
			case 'unlistedEntries':
				return tn('settings.backup.warning.unlistedEntries', warning.names.length);
		}
	});
}

/** The operations settings are one group on their page, however the file lists them. */
function groupLabel(group: string): string {
	const id = GROUP_LABELS[group];
	return id ? t(id) : group;
}

/**
 * Export and import of the settings. Both open the system's file dialogs from Rust; the page
 * shows where an export went, and shows what an import would change before anything does. Import
 * asks first, with Cancel as the default, and applies the plan Rust holds, never a copy of the file.
 */
export function BackupGroup() {
	const { transfer, errors, setRowError, refreshOps } = useSettingsEditor();
	const [busy, setBusy] = useState(false);
	const [preview, setPreview] = useState<ImportPreview | null>(null);
	if (!transfer) return null;

	const exportSettings = async () => {
		setRowError('exportSettings', null);
		setBusy(true);
		try {
			const receipt = await transfer.exportSettings();
			if (receipt) showNotice(tf('settings.backup.exported', { path: receipt.path }));
		} catch (error) {
			console.warn('could not export the settings', error);
			setRowError('exportSettings', transferErrorMessage(error));
		} finally {
			setBusy(false);
		}
	};

	const chooseImport = async () => {
		setRowError('importSettings', null);
		setBusy(true);
		try {
			const planned = await transfer.planImport();
			if (!planned) return;
			if (planned.plan.changes.length === 0) {
				showNotice(t('settings.backup.nothingToChange'));
				return;
			}
			setPreview(planned);
		} catch (error) {
			console.warn('could not read the settings file', error);
			setRowError('importSettings', transferErrorMessage(error));
		} finally {
			setBusy(false);
		}
	};

	const applyImport = async (planned: ImportPreview) => {
		setPreview(null);
		setBusy(true);
		try {
			await transfer.applyImport(planned.planId);
			// The settings document reaches the page as an event; the operations settings, which the
			// page holds its own copy of, are read again.
			await refreshOps();
			showNotice(t('settings.backup.imported'));
		} catch (error) {
			console.warn('could not import the settings', error);
			setRowError('importSettings', transferErrorMessage(error));
		} finally {
			setBusy(false);
		}
	};

	const plan = preview?.plan;
	return (
		<SettingsGroup title={t('settings.group.backup')}>
			<ButtonRow
				label={t('settings.backup.export.label')}
				description={t('settings.backup.export.description')}
				actionLabel={t('settings.backup.export.action')}
				error={errors.exportSettings}
				disabled={busy}
				onAction={() => void exportSettings()}
			/>
			<ButtonRow
				label={t('settings.backup.import.label')}
				description={t('settings.backup.import.description')}
				actionLabel={t('settings.backup.import.action')}
				error={errors.importSettings}
				disabled={busy}
				onAction={() => void chooseImport()}
			/>
			<Dialog
				open={preview !== null}
				title={t('settings.backup.confirm.title')}
				description={t('settings.backup.confirm.description')}
				size="medium"
				onClose={() => setPreview(null)}
				footer={
					<DialogActions>
						<DialogButton variant="secondary" onClick={() => setPreview(null)}>
							{t('settings.backup.confirm.cancel')}
						</DialogButton>
						<DialogButton variant="danger" onClick={() => preview && void applyImport(preview)}>
							{t('settings.backup.confirm.replace')}
						</DialogButton>
					</DialogActions>
				}
			>
				{plan ? (
					<div className={styles.plan}>
						{plan.appVersion ? (
							<p className={styles.source}>
								{tf('settings.backup.confirm.source', { version: plan.appVersion })}
							</p>
						) : null}
						<h3 className={styles.heading}>{t('settings.backup.confirm.changes')}</h3>
						<ul className={styles.list}>
							{plan.changes.map((change) => (
								<li key={`${change.file}:${change.group}`}>
									<span className={styles.group}>{groupLabel(change.group)}</span>
									<span className={styles.count}>
										{tn('settings.backup.changes', change.count)}
									</span>
								</li>
							))}
						</ul>
						{warningLines(plan).length > 0 ? (
							<ul className={styles.warnings}>
								{warningLines(plan).map((line) => (
									<li key={line}>{line}</li>
								))}
							</ul>
						) : null}
					</div>
				) : null}
			</Dialog>
		</SettingsGroup>
	);
}
