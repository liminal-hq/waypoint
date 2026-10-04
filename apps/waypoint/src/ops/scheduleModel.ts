// A job's schedule as the form edits it and as a row words it: the inputs it turns into a `Schedule`, and the sentence for one
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { HourCycle } from '../services/timeFormatClient';
import { formatLocale } from '../i18n/active';
import { tf, type MessageId } from '../i18n/messages';
import type { Schedule } from '../services/opsClient';

/** What the form holds: the kind chosen and the text of its fields, as the inputs give them. */
export interface ScheduleDraft {
	mode: 'startAt' | 'window';
	/** `datetime-local`'s value, "2026-10-05T22:30", in the person's own time zone. */
	startAt: string;
	/** `time`'s values, "22:00". */
	from: string;
	until: string;
}

export type ScheduleProblem = 'past' | 'same' | 'incomplete';

export const SCHEDULE_PROBLEM_MESSAGES: Record<ScheduleProblem, MessageId> = {
	past: 'ops.schedule.error.past',
	same: 'ops.schedule.error.same',
	incomplete: 'ops.schedule.error.incomplete',
};

/** Minutes the local time is ahead of UTC at `at`. `Date` counts the other way round. */
export function utcOffsetMinutes(at: Date = new Date()): number {
	// `getTimezoneOffset` is UTC minus local, and is -0 in UTC; the `+ 0` makes it 0.
	return -at.getTimezoneOffset() + 0;
}

function minutesOfDay(text: string): number | null {
	const match = /^(\d{1,2}):(\d{2})/.exec(text);
	if (!match) return null;
	const [hours, minutes] = [Number(match[1]), Number(match[2])];
	return hours < 24 && minutes < 60 ? hours * 60 + minutes : null;
}

/** The schedule a draft stands for, or what is wrong with it. `now` is in milliseconds. */
export function scheduleFromDraft(
	draft: ScheduleDraft,
	now: number,
): { schedule: Schedule } | { problem: ScheduleProblem } {
	if (draft.mode === 'startAt') {
		const at = draft.startAt === '' ? NaN : new Date(draft.startAt).getTime();
		if (Number.isNaN(at)) return { problem: 'incomplete' };
		if (at <= now) return { problem: 'past' };
		return { schedule: { kind: 'startAt', atMs: at } };
	}
	const start = minutesOfDay(draft.from);
	const end = minutesOfDay(draft.until);
	if (start === null || end === null) return { problem: 'incomplete' };
	if (start === end) return { problem: 'same' };
	return {
		schedule: {
			kind: 'window',
			startMinute: start,
			endMinute: end,
			utcOffsetMinutes: utcOffsetMinutes(new Date(now)),
		},
	};
}

const pad = (n: number) => String(n).padStart(2, '0');

/** `datetime-local`'s value for a moment, in local time. */
export function localDateTime(ms: number): string {
	const d = new Date(ms);
	return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function timeOfDayText(minute: number): string {
	return `${pad(Math.floor(minute / 60))}:${pad(minute % 60)}`;
}

/** The draft that shows what a schedule says, or a sensible start for a job with none (an hour from now, and an evening window). */
export function draftOf(schedule: Schedule | null, now: number): ScheduleDraft {
	if (schedule?.kind === 'window') {
		return {
			mode: 'window',
			startAt: localDateTime(now + 3_600_000),
			from: timeOfDayText(schedule.startMinute),
			until: timeOfDayText(schedule.endMinute),
		};
	}
	return {
		mode: 'startAt',
		startAt: localDateTime(schedule?.kind === 'startAt' ? schedule.atMs : now + 3_600_000),
		from: '22:00',
		until: '06:00',
	};
}

const timeFormats = new Map<string, Intl.DateTimeFormat>();

/** A time of day ("10:00 PM", "22:00") in the system's hour cycle, not tied to any date or zone. */
export function formatTimeOfDay(minute: number, locale?: string, hourCycle?: HourCycle): string {
	const tag = locale ?? formatLocale();
	const key = `${tag ?? ''}|${hourCycle ?? ''}`;
	let format = timeFormats.get(key);
	if (!format) {
		format = new Intl.DateTimeFormat(tag, { timeStyle: 'short', hourCycle, timeZone: 'UTC' });
		timeFormats.set(key, format);
	}
	return format.format(new Date(Date.UTC(2000, 0, 1, Math.floor(minute / 60), minute % 60)));
}

const dateFormats = new Map<string, Intl.DateTimeFormat>();

function formatMoment(ms: number, locale?: string, hourCycle?: HourCycle): string {
	const tag = locale ?? formatLocale();
	const key = `${tag ?? ''}|${hourCycle ?? ''}`;
	let format = dateFormats.get(key);
	if (!format) {
		format = new Intl.DateTimeFormat(tag, { dateStyle: 'medium', timeStyle: 'short', hourCycle });
		dateFormats.set(key, format);
	}
	return format.format(new Date(ms));
}

/** "Starts Oct 5, 2026, 10:30 PM" or "Only between 10:00 PM and 6:00 AM each day". */
export function scheduleText(schedule: Schedule, locale?: string, hourCycle?: HourCycle): string {
	return schedule.kind === 'startAt'
		? tf('ops.schedule.startsAt', { time: formatMoment(schedule.atMs, locale, hourCycle) })
		: tf('ops.schedule.window', {
				from: formatTimeOfDay(schedule.startMinute, locale, hourCycle),
				to: formatTimeOfDay(schedule.endMinute, locale, hourCycle),
			});
}
