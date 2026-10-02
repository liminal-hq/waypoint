// What the palette lists for a query: the registry's commands and the history's entries, matched, ranked and worded
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ComponentType } from 'react';
import { t, tf, type MessageId } from '../i18n/messages';
import type { IconProps } from '../icons/AppIcons';
import type { HourCycle } from '../services/timeFormatClient';
import { fuzzyMatch, type MatchRange } from './fuzzy';
import { exactTime, relativeTime, type HistoryRow } from './historyCommands';
import { GROUP_ORDER, type CommandId, type CommandView } from './registry';
import { pluralLocale } from '../i18n/active';

/** How many of the commands run last come first in an empty palette. */
export const RECENT_LIMIT = 5;

/** What choosing a row does: run a registry command, or apply a history chain. */
export type PaletteTarget =
	{ kind: 'command'; id: CommandId } | { kind: 'history'; row: HistoryRow };

export interface PaletteRow {
	/** Unique in the list; the option's id and the active row's identity. */
	key: string;
	label: string;
	/** Where `label` matched the query, for drawing those characters bold. */
	ranges: MatchRange[];
	/** The muted hint after the label: the command's group, "Recent" or "History". */
	hint: string;
	shortcut: string | undefined;
	/** When a history entry happened, as the row says it. */
	detail: string | undefined;
	/** The exact time behind `detail`, for a tooltip. */
	detailTitle: string | undefined;
	icon: ComponentType<IconProps> | undefined;
	enabled: boolean;
	/** Why a disabled row cannot run. */
	reason: string | undefined;
	checked: boolean | undefined;
	/** What a screen reader says for the option: the words the row shows, in one sentence. */
	name: string;
	target: PaletteTarget;
}

export interface PaletteInput {
	/** The registry resolved for the window (`useCommands().commands`). */
	commands: readonly CommandView[];
	/** The history's rows, which only a query brings in. */
	history: readonly HistoryRow[];
	query: string;
	/** The commands run last, newest first. */
	recents: readonly CommandId[];
	now?: number;
	locale?: string;
	hourCycle?: HourCycle;
}

const GROUP_LABELS: Record<CommandView['group'], MessageId> = {
	file: 'palette.group.file',
	edit: 'palette.group.edit',
	view: 'palette.group.view',
	go: 'palette.group.go',
	tabs: 'palette.group.tabs',
	window: 'palette.group.window',
	app: 'palette.group.app',
};

/** The commands the palette shows: the offered ones, and not the palette itself (it is open). */
export function listable(commands: readonly CommandView[]): CommandView[] {
	return commands.filter((view) => view.visible && view.id !== 'commandPalette');
}

function commandRow(view: CommandView, hint: string, ranges: MatchRange[]): PaletteRow {
	const parts = [view.label, hint];
	if (view.shortcut) parts.push(tf('palette.shortcut', { keys: view.shortcut }));
	if (view.checked !== undefined)
		parts.push(t(view.checked ? 'palette.checked' : 'palette.unchecked'));
	if (!view.enabled && view.reason) parts.push(tf('palette.unavailable', { reason: view.reason }));
	return {
		key: `command:${view.id}`,
		label: view.label,
		ranges,
		hint,
		shortcut: view.shortcut,
		detail: undefined,
		detailTitle: undefined,
		icon: view.icon,
		enabled: view.enabled,
		reason: view.reason,
		checked: view.checked,
		name: parts.join(', '),
		target: { kind: 'command', id: view.id },
	};
}

function historyRow(row: HistoryRow, ranges: MatchRange[], input: PaletteInput): PaletteRow {
	const now = input.now ?? Date.now();
	const detail = relativeTime(row.entry.atMs, now, input.locale, input.hourCycle);
	const note = row.partly ? `${detail}, ${t('history.row.partly')}` : detail;
	return {
		key: row.key,
		label: row.label,
		ranges,
		hint: t('palette.group.history'),
		shortcut: undefined,
		detail: note,
		detailTitle: exactTime(row.entry.atMs, input.locale, input.hourCycle),
		icon: undefined,
		enabled: true,
		reason: undefined,
		checked: undefined,
		name: tf('history.row.name', { row: row.label, time: note }),
		target: { kind: 'history', row },
	};
}

/**
 * The rows for `query`. Empty, it is the commands run last (up to `RECENT_LIMIT`, newest first),
 * then every other command in the menus' order. Typed, it is every command and history entry the
 * query matches as a subsequence of its label, best score first. Ties go to a recent command, then
 * to the group order (file, edit, view, go, tabs, window, app, then the history) and then to the
 * registry's own order, so the list never reshuffles for equal scores.
 */
export function paletteRows(input: PaletteInput): PaletteRow[] {
	const commands = listable(input.commands);
	const query = input.query.trim();
	const hintOf = (view: CommandView) => t(GROUP_LABELS[view.group]);

	if (query === '') {
		const byId = new Map(commands.map((view) => [view.id, view] as const));
		const recent = input.recents.flatMap((id) => byId.get(id) ?? []).slice(0, RECENT_LIMIT);
		const taken = new Set(recent.map((view) => view.id));
		const rest = commands
			.map((view, index) => ({ view, index }))
			.filter(({ view }) => !taken.has(view.id))
			.sort(
				(a, b) =>
					GROUP_ORDER.indexOf(a.view.group) - GROUP_ORDER.indexOf(b.view.group) ||
					a.index - b.index,
			)
			.map(({ view }) => view);
		return [
			...recent.map((view) => commandRow(view, t('palette.group.recent'), [])),
			...rest.map((view) => commandRow(view, hintOf(view), [])),
		];
	}

	interface Ranked {
		row: PaletteRow;
		score: number;
		recent: number;
		group: number;
		index: number;
	}
	const ranked: Ranked[] = [];
	commands.forEach((view, index) => {
		const match = fuzzyMatch(query, view.label);
		if (!match) return;
		const recent = input.recents.indexOf(view.id);
		ranked.push({
			row: commandRow(view, hintOf(view), match.ranges),
			score: match.score,
			recent: recent < 0 ? Infinity : recent,
			group: GROUP_ORDER.indexOf(view.group),
			index,
		});
	});
	input.history.forEach((row, index) => {
		const match = fuzzyMatch(query, row.label);
		if (!match) return;
		ranked.push({
			row: historyRow(row, match.ranges, input),
			score: match.score,
			recent: Infinity,
			group: GROUP_ORDER.length,
			index: commands.length + index,
		});
	});
	ranked.sort(
		(a, b) => b.score - a.score || a.recent - b.recent || a.group - b.group || a.index - b.index,
	);
	return ranked.map((entry) => entry.row);
}

/** The count the live region announces after the list changes. */
export function countText(count: number): string {
	if (count === 0) return t('palette.count.none');
	const form = new Intl.PluralRules(pluralLocale()).select(count) === 'one' ? 'one' : 'other';
	return tf(`palette.count.${form}`, { count });
}

/**
 * Moves the active row by `by` (`Infinity` and `-Infinity` go to the ends). With `wrap`, a step
 * off either end lands on the other, as the arrow keys do; a page step stops at the end.
 */
export function moveActive(current: number, by: number, count: number, wrap = false): number {
	if (count === 0) return -1;
	if (by === Infinity) return count - 1;
	if (by === -Infinity) return 0;
	const next = current + by;
	if (wrap) return ((next % count) + count) % count;
	return Math.min(count - 1, Math.max(0, next));
}
