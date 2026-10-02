// The application menu's items: the registry's commands arranged under File, Edit, View and Window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { JournalEntrySummary } from '@liminal-hq/waypoint-protocol/generated/JournalEntrySummary';
import { createElement, type ReactNode } from 'react';
import { t, tf } from '../i18n/messages';
import {
	CommandPaletteIcon,
	HistoryIcon,
	EditIcon,
	EyeIcon,
	FolderOpenIcon,
	WindowIcon,
} from '../icons/MenuIcons';
import type { CommandFacts } from './commandEnv';
import { commandDef, viewOf, type CommandId, type CommandView } from './registry';

/** The submenu rows' ids, which the menu's `onSelect` maps back to what to do. */
export const MENU_IDS = {
	file: 'menu:file',
	edit: 'menu:edit',
	view: 'menu:view',
	window: 'menu:window',
	history: 'menu:history',
} as const;

/**
 * Alt plus the letter opens that menu: `Alt+F` File, `Alt+E` Edit, `Alt+V` View and `Alt+W`
 * Window. There is no Help menu (Waypoint has no help destination yet), so `H` is not bound.
 */
export const APP_MENU_MNEMONICS: Readonly<Record<string, string>> = {
	f: MENU_IDS.file,
	e: MENU_IDS.edit,
	v: MENU_IDS.view,
	w: MENU_IDS.window,
};

/** The row of a command: hidden commands have none, and an unavailable one is disabled with its reason as the tooltip. */
export function commandRow(view: CommandView): MenuItem[] {
	if (!view.visible) return [];
	const common = {
		id: view.id,
		label: view.label,
		...(view.icon ? { icon: createElement(view.icon) } : {}),
		...(view.shortcut ? { shortcut: view.shortcut } : {}),
		...(view.enabled ? {} : { disabled: true }),
		...(view.reason ? { title: view.reason } : {}),
	};
	return [
		view.checked === undefined
			? { type: 'action', ...common }
			: { type: 'checkbox', checked: view.checked, ...common },
	];
}

/** Joins non-empty sections with one separator between them, so hiding a section never leaves a stray rule. */
export function menuSections(...groups: MenuItem[][]): MenuItem[] {
	const items: MenuItem[] = [];
	for (const group of groups) {
		if (group.length === 0) continue;
		if (items.length > 0) items.push({ type: 'separator', id: `sep-${items.length}` });
		items.push(...group);
	}
	return items;
}

/** "14:32" for today's entries and "Oct 1, 14:32" for older ones. */
export function historyTime(atMs: number, now: number = Date.now(), locale?: string): string {
	const at = new Date(atMs);
	const sameDay = at.toDateString() === new Date(now).toDateString();
	return new Intl.DateTimeFormat(
		locale,
		sameDay
			? { hour: 'numeric', minute: '2-digit' }
			: { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' },
	).format(at);
}

/** The history submenu's footer, which opens the palette on "undo" where every entry can be chosen. */
export const HISTORY_MORE_ID = 'history:more';

/** What selecting a history row does. */
export type HistoryRowKind = 'undo' | 'redo';

export const historyRowId = (kind: HistoryRowKind, entry: number) => `history:${kind}:${entry}`;

/** Reads a history row's id back, or `null` for any other id. */
export function parseHistoryRow(id: string): { kind: HistoryRowKind; entry: number } | null {
	const match = /^history:(undo|redo):(\d+)$/.exec(id);
	return match ? { kind: match[1] as HistoryRowKind, entry: Number(match[2]) } : null;
}

/**
 * The Undo History submenu: the journal's entries, newest first, each with when it happened (an
 * undone entry says so). Only the entry Undo would undo and the entry Redo would redo can be
 * chosen: the rest are listed so the history can be read, and disabled, because an older change
 * may be built on a newer one. The palette offers every entry.
 */
export function historyItems(
	facts: Pick<CommandFacts, 'history' | 'undoHead' | 'redoHead'>,
	now: number = Date.now(),
	locale?: string,
): MenuItem[] {
	if (facts.history.length === 0) {
		return [
			{
				type: 'action',
				id: 'history:empty',
				label: t('appMenu.history.empty'),
				icon: createElement(HistoryIcon),
				disabled: true,
			},
		];
	}
	const entries = facts.history.map((entry: JournalEntrySummary): MenuItem => {
		const time = historyTime(entry.atMs, now, locale);
		const kind: HistoryRowKind | null =
			entry.id === facts.undoHead ? 'undo' : entry.id === facts.redoHead ? 'redo' : null;
		return {
			type: 'action',
			id: kind ? historyRowId(kind, entry.id) : `history:later:${entry.id}`,
			label: tf(entry.redoable ? 'appMenu.history.undone' : 'appMenu.history.entry', {
				label: entry.label,
				time,
			}),
			icon: createElement(HistoryIcon),
			...(kind ? {} : { disabled: true, title: t('appMenu.history.later') }),
		};
	});
	return [
		...entries,
		{ type: 'separator', id: 'history:rule' },
		{
			type: 'action',
			id: HISTORY_MORE_ID,
			label: t('appMenu.history.more'),
			icon: createElement(CommandPaletteIcon),
		},
	];
}

/**
 * The menu: File, Edit, View and Window, each only when it has something to offer. `views` is the
 * registry evaluated for the window (`evaluateCommands`).
 */
export function appMenuItems(
	views: readonly CommandView[],
	facts: CommandFacts,
	now: number = Date.now(),
	locale?: string,
): MenuItem[] {
	const byId = new Map(views.map((view) => [view.id, view] as const));
	const rows = (...ids: CommandId[]): MenuItem[] =>
		ids.flatMap((id) => commandRow(byId.get(id) ?? viewOf(commandDef(id), facts)));

	const file = menuSections(
		rows('newWindow', 'newTab'),
		rows('newFolder', 'newFile'),
		rows('rename', 'batchRename', 'duplicate'),
		rows('moveToTrash', 'deletePermanently'),
		rows('closeTab', 'reopenClosedTab', 'closeWindow'),
	);
	const edit = menuSections(
		[
			...rows('undo', 'redo'),
			{
				type: 'submenu',
				id: MENU_IDS.history,
				label: t('appMenu.history'),
				icon: createElement(HistoryIcon),
				items: historyItems(facts, now, locale),
			} satisfies MenuItem,
		].filter((item) => item.type !== 'submenu' || byId.get('undo')?.visible),
		rows('cut', 'copy', 'paste', 'copyTo', 'moveTo', 'linkTo'),
		rows('selectAll', 'invertSelection'),
	);
	const view = menuSections(
		rows('viewList', 'viewGrid'),
		rows('showHidden', 'sidebar', 'actionBar'),
		rows('splitView'),
		rows('commandPalette'),
	);
	const window = menuSections(
		rows('duplicateTab', 'moveTabToNewWindow'),
		rows('alwaysOnTop'),
		rows('settings'),
	);

	const menus: Array<{ id: string; label: string; icon: ReactNode; items: MenuItem[] }> = [
		{
			id: MENU_IDS.file,
			label: t('appMenu.file'),
			icon: createElement(FolderOpenIcon),
			items: file,
		},
		{ id: MENU_IDS.edit, label: t('appMenu.edit'), icon: createElement(EditIcon), items: edit },
		{ id: MENU_IDS.view, label: t('appMenu.view'), icon: createElement(EyeIcon), items: view },
		{
			id: MENU_IDS.window,
			label: t('appMenu.window'),
			icon: createElement(WindowIcon),
			items: window,
		},
	];
	return menus.flatMap((menu) =>
		menu.items.length === 0 ? [] : [{ type: 'submenu', ...menu } satisfies MenuItem],
	);
}
