// Sizes and dates for the list, formatted with `Intl` in the person's locale
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { HourCycle } from '../services/timeFormatClient';

const UNITS = ['byte', 'kilobyte', 'megabyte', 'gigabyte', 'terabyte', 'petabyte'] as const;

const numberFormats = new Map<string, Intl.NumberFormat>();
const dateFormats = new Map<string, Intl.DateTimeFormat>();

function sizeFormat(locale: string | undefined, unit: string, digits: number): Intl.NumberFormat {
	const key = `${locale ?? ''}|${unit}|${digits}`;
	let format = numberFormats.get(key);
	if (!format) {
		format = new Intl.NumberFormat(locale, {
			style: 'unit',
			unit,
			unitDisplay: 'short',
			maximumFractionDigits: digits,
		});
		numberFormats.set(key, format);
	}
	return format;
}

/**
 * A byte count in the largest unit that keeps it under 1000 (decimal units, as GNOME Files shows),
 * with one decimal below ten and none above.
 */
export function formatSize(bytes: number, locale?: string): string {
	let value = Math.max(0, bytes);
	let unit = 0;
	while (value >= 1000 && unit < UNITS.length - 1) {
		value /= 1000;
		unit += 1;
	}
	let digits = unit > 0 && value < 10 ? 1 : 0;
	// Rounding can carry a value to 1000 (999_999 bytes): promote it rather than print "1,000 kB".
	const factor = 10 ** digits;
	if (Math.round(value * factor) / factor >= 1000 && unit < UNITS.length - 1) {
		value /= 1000;
		unit += 1;
		digits = value < 10 ? 1 : 0;
	}
	return sizeFormat(locale, UNITS[unit]!, digits).format(value);
}

/**
 * A modification time (milliseconds since the Unix epoch) as a short date and time. `hourCycle` is
 * the system's 12/24-hour setting; without it the locale decides, which is wrong for a person whose
 * clock setting differs from their locale's convention (`en-CA` defaults to 12-hour).
 */
export function formatModified(modifiedMs: number, locale?: string, hourCycle?: HourCycle): string {
	const key = `${locale ?? ''}|${hourCycle ?? ''}`;
	let format = dateFormats.get(key);
	if (!format) {
		format = new Intl.DateTimeFormat(locale, {
			dateStyle: 'medium',
			timeStyle: 'short',
			hourCycle,
		});
		dateFormats.set(key, format);
	}
	return format.format(new Date(modifiedMs));
}
