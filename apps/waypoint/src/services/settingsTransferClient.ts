// Saving the settings to a file and loading them back, as the Settings page uses them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ExportReceipt, ImportPreview } from '@liminal-hq/waypoint-plugin-settings';
import type { SettingsSnapshot } from './settingsClient';

export type { ExportReceipt, ImportPreview };

/**
 * Everything the Back up and restore rows ask of the settings plugin. The file dialogs open in
 * Rust and the file stays there: the page learns where an export went and what an import would
 * change, and applies an import by the number of the plan it just saw.
 */
export interface SettingsTransferClient {
	/** Asks where to save and writes the file; `null` when the dialog was closed. */
	exportSettings(): Promise<ExportReceipt | null>;
	/** Asks which file to read and says what importing it would change; `null` when the dialog was closed. */
	planImport(): Promise<ImportPreview | null>;
	/** Applies the plan just made, all or nothing, and returns the settings now in force. */
	applyImport(planId: number): Promise<SettingsSnapshot>;
}
