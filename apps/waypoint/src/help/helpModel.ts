// What the Help dialog, the shortcut list and the tour say, read from the command registry so a key is never written twice
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t, tf, type MessageId } from '../i18n/messages';
import {
	commandDef,
	GROUP_ORDER,
	type CommandGroup,
	type CommandId,
	type CommandView,
} from '../commands/registry';
import type { HelpPage } from './helpPages';

/** The commands Help picks out, in order, each with the plain sentence that says what its key does. */
export const HELP_ROWS: ReadonlyArray<{ id: CommandId; text: MessageId }> = [
	{ id: 'commandPalette', text: 'help.row.commandPalette' },
	{ id: 'newTab', text: 'help.row.newTab' },
	{ id: 'splitView', text: 'help.row.splitView' },
	{ id: 'undo', text: 'help.row.undo' },
	{ id: 'toggleShelf', text: 'help.row.toggleShelf' },
	{ id: 'keyboardShortcuts', text: 'help.row.keyboardShortcuts' },
];

export interface HelpRow {
	id: CommandId;
	/** What the key does, in a sentence. */
	text: string;
	/** The key as the registry shows it ("Ctrl+Shift+P"). */
	keys: string;
}

/** Whether a command is offered here and has a key to show. */
function keyed(view: CommandView | undefined): view is CommandView & { shortcut: string } {
	return view !== undefined && view.visible && view.shortcut !== undefined;
}

/** Help's rows: each picked command that is offered in this window and has a key, so nothing it cannot do is described. */
export function helpRows(views: readonly CommandView[]): HelpRow[] {
	const byId = new Map(views.map((view) => [view.id, view] as const));
	return HELP_ROWS.flatMap(({ id, text }) => {
		const view = byId.get(id);
		return keyed(view) ? [{ id, text: t(text), keys: view.shortcut }] : [];
	});
}

export interface ShortcutRow {
	id: CommandId;
	/** The command's own name, not what it would do now ("Undo", never "Undo Rename"). */
	label: string;
	keys: string;
}

export interface ShortcutGroup {
	group: CommandGroup;
	label: string;
	rows: ShortcutRow[];
}

/**
 * Every offered command that has a key, under its group and in the registry's order. A command
 * the window hides (the Trash's Restore in a folder) is left out, and a disabled one stays: its
 * key is still its key.
 */
export function shortcutGroups(views: readonly CommandView[]): ShortcutGroup[] {
	return GROUP_ORDER.flatMap((group) => {
		const rows = views.flatMap((view): ShortcutRow[] =>
			view.group === group && keyed(view)
				? [{ id: view.id, label: t(commandDef(view.id).label), keys: view.shortcut }]
				: [],
		);
		return rows.length === 0 ? [] : [{ group, label: t(`palette.group.${group}`), rows }];
	});
}

/** One step of the tour: its heading, and the text with the keys of the commands it mentions filled in. */
export interface TourStep {
	title: string;
	text: string;
}

const STEPS: ReadonlyArray<{
	title: MessageId;
	text: MessageId;
	keys?: Readonly<Record<string, CommandId>>;
}> = [
	{ title: 'tour.title.welcome', text: 'tour.text.welcome' },
	{
		title: 'tour.title.tabs',
		text: 'tour.text.tabs',
		keys: { split: 'splitView', reopen: 'reopenClosedTab' },
	},
	{ title: 'tour.title.dnd', text: 'tour.text.dnd', keys: { shelf: 'toggleShelf' } },
	{
		title: 'tour.title.commands',
		text: 'tour.text.commands',
		keys: { palette: 'commandPalette', shortcuts: 'keyboardShortcuts' },
	},
	{ title: 'tour.title.desktop', text: 'tour.text.desktop' },
];

/** How many steps the tour has. */
export const TOUR_LENGTH = STEPS.length;

/** The tour's steps. A key comes from the registry; a command with none is named instead. */
export function tourSteps(): TourStep[] {
	return STEPS.map((step) => ({
		title: t(step.title),
		text: tf(
			step.text,
			Object.fromEntries(
				Object.entries(step.keys ?? {}).map(([token, id]) => {
					const def = commandDef(id);
					return [token, def.shortcut ?? t(def.label)];
				}),
			),
		),
	}));
}

type KeyEventLike = Pick<
	KeyboardEvent,
	'key' | 'ctrlKey' | 'metaKey' | 'altKey' | 'isComposing' | 'target'
>;

const NON_TEXT_INPUTS = new Set([
	'button',
	'checkbox',
	'color',
	'file',
	'image',
	'radio',
	'range',
	'reset',
	'submit',
]);

/** Whether the key is going into a text field, where `?` is a character and not a command. */
export function isTextEntry(target: EventTarget | null): boolean {
	if (!(target instanceof Element)) return false;
	const field = target.closest('input, textarea, select, [contenteditable]');
	if (!field) return false;
	if (field instanceof HTMLInputElement) return !NON_TEXT_INPUTS.has(field.type);
	if (field instanceof HTMLElement && field.hasAttribute('contenteditable')) {
		return field.getAttribute('contenteditable') !== 'false';
	}
	return true;
}

/**
 * The page a key opens: F1 is Help, anywhere; `?` (Shift+/) is the shortcut list, except in a
 * text field. Neither takes Ctrl, Alt or Cmd, and the other modifiers are the keyboard's own
 * way of making the key.
 */
export function helpPageForKey(event: KeyEventLike): HelpPage | null {
	if (event.isComposing || event.ctrlKey || event.metaKey || event.altKey) return null;
	if (event.key === 'F1') return 'help';
	if (event.key === '?' && !isTextEntry(event.target)) return 'shortcuts';
	return null;
}
