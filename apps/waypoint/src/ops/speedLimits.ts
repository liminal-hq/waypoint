// The speed limits a job's row offers, and how a limit is worded
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t, tf } from '../i18n/messages';
import { BYTES_PER_MB } from '../settings/opsDefaults';

/** The speeds, in MB/s, a row's limit menu offers, besides no limit and a limit it already has. */
export const SPEED_LIMIT_CHOICES = [1, 2, 5, 10, 25, 50, 100] as const;

/** The choices for a job whose limit is `current` bytes a second: no limit, the usual speeds, and `current` if it is none of them (a limit another window set). */
export function speedLimitChoices(current: number | null): (number | null)[] {
	const choices: (number | null)[] = [null, ...SPEED_LIMIT_CHOICES.map((mb) => mb * BYTES_PER_MB)];
	if (current !== null && !choices.includes(current)) {
		choices.push(current);
		choices.sort((a, b) => (a ?? 0) - (b ?? 0));
	}
	return choices;
}

/** "No limit" or "10 MB/s". */
export function speedLimitLabel(bytes: number | null): string {
	if (bytes === null) return t('ops.speedLimit.none');
	const mb = bytes / BYTES_PER_MB;
	return tf('ops.speedLimit.value', { speed: Number.isInteger(mb) ? mb : mb.toFixed(1) });
}
