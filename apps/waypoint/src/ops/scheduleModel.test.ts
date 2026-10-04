// Verifies the schedule form's model: what a draft stands for, what is wrong with one, and the words for a schedule
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import {
	draftOf,
	formatTimeOfDay,
	localDateTime,
	scheduleFromDraft,
	scheduleText,
	timeOfDayText,
	utcOffsetMinutes,
} from './scheduleModel';

const NOW = new Date(2026, 9, 4, 12, 0, 0).getTime();

describe('a draft becomes a schedule', () => {
	it('takes a start time in the person’s own zone and refuses one that has passed', () => {
		const draft = draftOf(null, NOW);
		expect(draft).toMatchObject({ mode: 'startAt', from: '22:00', until: '06:00' });
		const future = { ...draft, startAt: '2026-10-05T22:30' };
		expect(scheduleFromDraft(future, NOW)).toEqual({
			schedule: { kind: 'startAt', atMs: new Date(2026, 9, 5, 22, 30).getTime() },
		});
		expect(scheduleFromDraft({ ...draft, startAt: '2026-10-04T11:59' }, NOW)).toEqual({
			problem: 'past',
		});
		expect(scheduleFromDraft({ ...draft, startAt: '' }, NOW)).toEqual({ problem: 'incomplete' });
	});

	it('takes a window as minutes of the day with the zone it was set in, across midnight too', () => {
		const draft = { ...draftOf(null, NOW), mode: 'window' as const };
		expect(scheduleFromDraft(draft, NOW)).toEqual({
			schedule: {
				kind: 'window',
				startMinute: 22 * 60,
				endMinute: 6 * 60,
				utcOffsetMinutes: utcOffsetMinutes(new Date(NOW)),
			},
		});
		expect(scheduleFromDraft({ ...draft, from: '09:00', until: '09:00' }, NOW)).toEqual({
			problem: 'same',
		});
		expect(scheduleFromDraft({ ...draft, from: '', until: '09:00' }, NOW)).toEqual({
			problem: 'incomplete',
		});
		expect(scheduleFromDraft({ ...draft, from: '25:00', until: '09:00' }, NOW)).toEqual({
			problem: 'incomplete',
		});
	});

	it('shows an existing schedule back in the form', () => {
		const window = {
			kind: 'window',
			startMinute: 90,
			endMinute: 330,
			utcOffsetMinutes: 0,
		} as const;
		expect(draftOf(window, NOW)).toMatchObject({ mode: 'window', from: '01:30', until: '05:30' });
		const at = new Date(2026, 9, 6, 8, 5).getTime();
		expect(draftOf({ kind: 'startAt', atMs: at }, NOW)).toMatchObject({
			mode: 'startAt',
			startAt: '2026-10-06T08:05',
		});
		expect(localDateTime(at)).toBe('2026-10-06T08:05');
		expect(timeOfDayText(5)).toBe('00:05');
	});

	it('counts the zone as minutes ahead of UTC, as Rust does', () => {
		expect(utcOffsetMinutes(new Date(NOW))).toBe(-new Date(NOW).getTimezoneOffset() + 0);
	});
});

describe('the words for a schedule', () => {
	it('say when a job starts and which hours a window covers, in the system clock', () => {
		expect(formatTimeOfDay(22 * 60, 'en-CA', 'h23')).toMatch(/^22:00$/);
		expect(formatTimeOfDay(6 * 60 + 30, 'en-CA', 'h12')).toMatch(/^6:30\s?a\.m\.$/);
		const window = {
			kind: 'window',
			startMinute: 1320,
			endMinute: 360,
			utcOffsetMinutes: 0,
		} as const;
		expect(scheduleText(window, 'en-CA', 'h23')).toBe('Scheduled between 22:00 and 06:00 each day');
		const at = new Date(2026, 9, 5, 22, 30).getTime();
		expect(scheduleText({ kind: 'startAt', atMs: at }, 'en-CA', 'h23')).toMatch(
			/^Scheduled for .*2026.*22:30$/,
		);
	});
});
