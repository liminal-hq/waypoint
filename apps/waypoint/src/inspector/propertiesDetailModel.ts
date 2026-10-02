// The pure rules of the Properties window's extra detail: permissions taken apart, and a time written out in full
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { HourCycle } from '../services/timeFormatClient';
import { formatLocale } from '../i18n/active';

export type PermissionWho = 'owner' | 'group' | 'others';

export interface PermissionRow {
	who: PermissionWho;
	read: boolean;
	write: boolean;
	execute: boolean;
}

/** The nine permission bits as three rows of read, write and execute. */
export function permissionRows(mode: number): PermissionRow[] {
	const row = (who: PermissionWho, shift: number): PermissionRow => ({
		who,
		read: (mode & (0o4 << shift)) !== 0,
		write: (mode & (0o2 << shift)) !== 0,
		execute: (mode & (0o1 << shift)) !== 0,
	});
	return [row('owner', 6), row('group', 3), row('others', 0)];
}

export type SpecialBit = 'setuid' | 'setgid' | 'sticky';

/** The special bits that are set, in the order `ls` shows them. */
export function specialBits(mode: number): SpecialBit[] {
	const bits: SpecialBit[] = [];
	if ((mode & 0o4000) !== 0) bits.push('setuid');
	if ((mode & 0o2000) !== 0) bits.push('setgid');
	if ((mode & 0o1000) !== 0) bits.push('sticky');
	return bits;
}

/** The mode in octal with its special digit: `0755`, `4755`. */
export function octalMode(mode: number): string {
	return (mode & 0o7777).toString(8).padStart(4, '0');
}

/** A moment written out in full for the person's locale, to the second (the system's 12 or 24-hour choice applies). */
export function formatFullTime(ms: number, hourCycle?: HourCycle, locale?: string): string {
	return new Intl.DateTimeFormat(locale ?? formatLocale(), {
		dateStyle: 'full',
		timeStyle: 'medium',
		...(hourCycle ? { hour12: hourCycle === 'h12' } : {}),
	}).format(new Date(ms));
}

/** The same moment as ISO 8601 in UTC, which is what a script or a bug report wants. */
export function formatIsoTime(ms: number): string {
	return new Date(ms).toISOString();
}
