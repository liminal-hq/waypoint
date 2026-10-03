// An in-memory SettingsTransferClient: the dialogs answer as the test says, and a plan is spent when applied
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { SettingsCommandError, SettingsSnapshot } from './settingsClient';
import type {
	ExportReceipt,
	ImportPreview,
	SettingsTransferClient,
} from './settingsTransferClient';

export interface FakeSettingsTransfer extends SettingsTransferClient {
	/** How many times each command was called, and the plan numbers `applyImport` was given. */
	readonly calls: { export: number; plan: number; apply: number[] };
	/** What the next `exportSettings` answers: a receipt, `null` for a closed dialog, or a refusal. */
	nextExport(answer: ExportReceipt | null | SettingsCommandError): void;
	/** What the next `planImport` answers: a plan, `null` for a closed dialog, or a refusal. */
	nextPlan(answer: ImportPreview | null | SettingsCommandError): void;
	/** What the next `applyImport` answers: the settings now in force, or a refusal. */
	nextApply(answer: SettingsSnapshot | SettingsCommandError): void;
}

const isRefusal = (answer: unknown): answer is SettingsCommandError =>
	typeof answer === 'object' && answer !== null && 'kind' in answer && 'message' in answer;

export function createFakeSettingsTransferClient(): FakeSettingsTransfer {
	let exportAnswer: ExportReceipt | null | SettingsCommandError = null;
	let planAnswer: ImportPreview | null | SettingsCommandError = null;
	let applyAnswer: SettingsSnapshot | SettingsCommandError | undefined;
	const calls = { export: 0, plan: 0, apply: [] as number[] };
	return {
		calls,
		nextExport(answer) {
			exportAnswer = answer;
		},
		nextPlan(answer) {
			planAnswer = answer;
		},
		nextApply(answer) {
			applyAnswer = answer;
		},
		async exportSettings() {
			calls.export += 1;
			if (isRefusal(exportAnswer)) throw exportAnswer;
			return exportAnswer;
		},
		async planImport() {
			calls.plan += 1;
			if (isRefusal(planAnswer)) throw planAnswer;
			return planAnswer;
		},
		async applyImport(planId) {
			calls.apply.push(planId);
			const answer = applyAnswer;
			applyAnswer = undefined;
			// As in Rust, a plan that is not ready (or was already used) is stale.
			if (!answer) {
				const stale: SettingsCommandError = {
					kind: 'stale',
					message: 'that import is no longer the one ready to apply',
				};
				throw stale;
			}
			if (isRefusal(answer)) throw answer;
			return answer;
		},
	};
}
