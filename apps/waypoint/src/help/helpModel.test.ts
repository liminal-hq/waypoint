// Tests for what Help lists: the rows come from the registry, a command the window hides is left out, and F1 and `?` mean Help only where they should
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { COMMANDS, commandDef, evaluateCommands, type CommandView } from '../commands/registry';
import { t } from '../i18n/messages';
import { factsFor } from '../test/commandFacts';
import {
	HELP_ROWS,
	helpPageForKey,
	helpRows,
	isTextEntry,
	shortcutGroups,
	TOUR_LENGTH,
	tourSteps,
} from './helpModel';

const views = evaluateCommands(factsFor({ selected: 1, focused: true }));

const withoutCommand = (id: string): CommandView[] =>
	views.map((view) => (view.id === id ? { ...view, visible: false, enabled: false } : view));

describe('Help’s rows', () => {
	it('list each command’s key as the registry shows it, in Help’s order', () => {
		const rows = helpRows(views);
		expect(rows.map((row) => row.id)).toEqual(HELP_ROWS.map((row) => row.id));
		for (const row of rows) expect(row.keys).toBe(commandDef(row.id).shortcut);
		expect(rows.find((row) => row.id === 'commandPalette')?.keys).toBe('Ctrl+Shift+P');
		expect(rows.find((row) => row.id === 'keyboardShortcuts')?.keys).toBe('?');
	});

	it('leave out a command this window does not offer, and never mention a key the app lacks', () => {
		expect(helpRows(withoutCommand('splitView')).map((row) => row.id)).not.toContain('splitView');
		const text = helpRows(views)
			.map((row) => `${row.text} ${row.keys}`)
			.join(' ');
		expect(text).not.toMatch(/Ctrl\+K|Quick Look|Search|Ctrl\+Alt\+N/);
	});
});

describe('the shortcut list', () => {
	it('holds every offered command that has a key, once, under its group', () => {
		const listed = shortcutGroups(views).flatMap((group) => group.rows.map((row) => row.id));
		const wanted = views.filter((view) => view.visible && view.shortcut).map((view) => view.id);
		expect([...listed].sort()).toEqual([...wanted].sort());
		expect(new Set(listed).size).toBe(listed.length);
		expect(listed).toContain('help');
		expect(listed).toContain('keyboardShortcuts');
	});

	it('shows the key the registry gives, and the command’s name rather than what it would do now', () => {
		const rows = shortcutGroups(
			evaluateCommands(factsFor({ undo: { id: 1, label: 'Rename report' } as never })),
		).flatMap((group) => group.rows);
		const undo = rows.find((row) => row.id === 'undo');
		expect(undo).toEqual({ id: 'undo', label: t('menu.undo'), keys: 'Ctrl+Z' });
		for (const row of rows) expect(row.keys).toBe(commandDef(row.id).shortcut);
	});

	it('omits a command the window hides and a command with no key', () => {
		const ids = shortcutGroups(withoutCommand('newTab')).flatMap((g) => g.rows.map((r) => r.id));
		expect(ids).not.toContain('newTab');
		const keyless = COMMANDS.filter((command) => !command.shortcut).map((command) => command.id);
		for (const id of keyless) expect(ids).not.toContain(id);
	});

	it('is empty when nothing is offered', () => {
		expect(shortcutGroups([])).toEqual([]);
	});
});

describe('the tour’s steps', () => {
	it('are five, each with a heading and text', () => {
		const steps = tourSteps();
		expect(steps).toHaveLength(TOUR_LENGTH);
		expect(TOUR_LENGTH).toBe(5);
		for (const step of steps) {
			expect(step.title).not.toBe('');
			expect(step.text).not.toMatch(/\{\w+\}/);
		}
	});

	it('quote the keys from the registry', () => {
		const text = tourSteps()
			.map((step) => step.text)
			.join(' ');
		for (const id of [
			'splitView',
			'reopenClosedTab',
			'toggleShelf',
			'commandPalette',
			'keyboardShortcuts',
		] as const) {
			expect(text).toContain(commandDef(id).shortcut!);
		}
		expect(text).toMatch(/Ctrl to copy, Shift to move, or Alt to choose/);
	});
});

const key = (init: Partial<KeyboardEvent> & { key: string }, target: EventTarget | null = null) =>
	({
		ctrlKey: false,
		metaKey: false,
		altKey: false,
		isComposing: false,
		target,
		...init,
	}) as Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'altKey' | 'isComposing' | 'target'>;

describe('the keys that open Help', () => {
	it('F1 opens Help, even in a text field', () => {
		expect(helpPageForKey(key({ key: 'F1' }))).toBe('help');
		expect(helpPageForKey(key({ key: 'F1' }, document.createElement('input')))).toBe('help');
	});

	it('? opens the shortcut list outside a text field only', () => {
		expect(helpPageForKey(key({ key: '?' }))).toBe('shortcuts');
		expect(helpPageForKey(key({ key: '?' }, document.createElement('button')))).toBe('shortcuts');
		expect(helpPageForKey(key({ key: '?' }, document.createElement('input')))).toBeNull();
		expect(helpPageForKey(key({ key: '?' }, document.createElement('textarea')))).toBeNull();
		const editable = document.createElement('div');
		editable.setAttribute('contenteditable', 'true');
		expect(helpPageForKey(key({ key: '?' }, editable))).toBeNull();
	});

	it('does not take a key with Ctrl, Alt or Cmd, or one that is part of composing text', () => {
		expect(helpPageForKey(key({ key: 'F1', ctrlKey: true }))).toBeNull();
		expect(helpPageForKey(key({ key: '?', altKey: true }))).toBeNull();
		expect(helpPageForKey(key({ key: '?', metaKey: true }))).toBeNull();
		expect(helpPageForKey(key({ key: '?', isComposing: true }))).toBeNull();
		expect(helpPageForKey(key({ key: '/' }))).toBeNull();
	});

	it('counts a checkbox as no text field', () => {
		const box = document.createElement('input');
		box.type = 'checkbox';
		expect(isTextEntry(box)).toBe(false);
		expect(isTextEntry(null)).toBe(false);
	});
});
