// The Action bar's buttons as data: which registry commands each stands for, and how it looks now
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import { createElement, type ComponentType } from 'react';
import { commandRow, menuSections } from '../commands/appMenuModel';
import type { CommandsApi } from '../commands/commandBridge';
import type { CommandId } from '../commands/registry';
import { t, tf } from '../i18n/messages';
import { GridViewIcon, ListViewIcon, PlusIcon, type IconProps } from '../icons/AppIcons';
import { SortIcon } from '../icons/MenuIcons';

/** One button of the bar. A button either runs `command` or opens `menu`. */
export interface ActionBarItem {
	id: string;
	/** The visible text (when labels show) and the accessible name. */
	label: string;
	/** What the tooltip says: the name and the key, or why the button is disabled. */
	tooltip: string;
	icon: ComponentType<IconProps>;
	enabled: boolean;
	/** Why it is disabled, for the accessible description. */
	reason: string | undefined;
	/** The buttons of one group sit together; a rule is drawn between groups. */
	group: number;
	/** The command a click runs; absent on a menu button. */
	command?: CommandId;
	/** The rows a menu button opens. */
	menu?: MenuItem[];
}

/** A button's tooltip: the name with its key, and for a disabled one the reason it cannot be used. */
export function tooltipFor(name: string, shortcut: string | undefined, reason: string | undefined) {
	if (reason) return tf('actionBar.disabledBecause', { name, reason });
	return shortcut ? tf('actionBar.withShortcut', { name, keys: shortcut }) : name;
}

/** Whether a rule is drawn before `items[index]`: it starts a new group. */
export function dividerBefore(items: readonly ActionBarItem[], index: number): boolean {
	const previous = items[index - 1];
	return previous !== undefined && previous.group !== items[index]!.group;
}

const NEW_GROUP = 0;
const EDIT_GROUP = 1;
const VIEW_GROUP = 2;
const HISTORY_GROUP = 3;

/**
 * The bar: New (a menu of Folder and File), Cut, Copy, Paste, Rename, Delete (moves to the
 * Trash), Sort (a menu), View (switches between List and Grid), Undo and Redo. A button whose
 * command is hidden (nothing is written in the Trash) is left out, and a menu with nothing in it
 * too. Extract all arrives with archives; Share has no destination yet.
 */
export function actionBarItems(api: Pick<CommandsApi, 'get'>): ActionBarItem[] {
	const items: ActionBarItem[] = [];

	const newViews = (['newFolder', 'newFile'] as const)
		.map((id) => api.get(id))
		.filter((view) => view.visible);
	if (newViews.length > 0) {
		items.push({
			id: 'new',
			label: t('actionBar.new'),
			tooltip: t('actionBar.new'),
			icon: PlusIcon,
			enabled: newViews.some((view) => view.enabled),
			reason: undefined,
			group: NEW_GROUP,
			menu: newViews.flatMap(commandRow),
		});
	}

	const button = (id: CommandId, label: string, group: number) => {
		const view = api.get(id);
		if (!view.visible || !view.icon) return;
		items.push({
			id,
			label,
			tooltip: tooltipFor(id === 'moveToTrash' ? view.label : label, view.shortcut, view.reason),
			icon: view.icon,
			enabled: view.enabled,
			reason: view.reason,
			group,
			command: id,
		});
	};
	button('cut', t('menu.cut'), EDIT_GROUP);
	button('copy', t('menu.copy'), EDIT_GROUP);
	button('paste', t('menu.paste'), EDIT_GROUP);
	button('rename', t('menu.rename'), EDIT_GROUP);
	button('moveToTrash', t('actionBar.delete'), EDIT_GROUP);

	const sortRows = menuSections(
		(['sortName', 'sortSize', 'sortModified', 'sortKind', 'sortDeleted'] as const)
			.map((id) => api.get(id))
			.flatMap(commandRow),
		(['sortDescending', 'sortFoldersFirst'] as const).map((id) => api.get(id)).flatMap(commandRow),
	);
	if (sortRows.length > 0) {
		items.push({
			id: 'sort',
			label: t('actionBar.sort'),
			tooltip: t('actionBar.sort'),
			icon: SortIcon,
			enabled: true,
			reason: undefined,
			group: VIEW_GROUP,
			menu: sortRows,
		});
	}

	// The View button switches to the mode the window is not in.
	const toList = api.get('viewGrid').checked === true;
	const target = api.get(toList ? 'viewList' : 'viewGrid');
	const switchTo = tf('actionBar.viewTo', { name: target.label });
	items.push({
		id: 'view',
		label: t('actionBar.view'),
		tooltip: tooltipFor(switchTo, target.shortcut, target.reason),
		icon: toList ? GridViewIcon : ListViewIcon,
		enabled: target.enabled,
		reason: target.reason,
		group: VIEW_GROUP,
		command: target.id,
	});

	button('undo', t('menu.undo'), HISTORY_GROUP);
	button('redo', t('menu.redo'), HISTORY_GROUP);
	return items;
}

/**
 * The buttons that did not fit, as the rows of the More menu. A menu button becomes a submenu; a
 * command button its row, with the state the registry gives it.
 */
export function overflowRows(overflowed: readonly ActionBarItem[]): MenuItem[] {
	return overflowed.map((item): MenuItem => {
		const icon = createElement(item.icon);
		if (item.menu) {
			return { type: 'submenu', id: `bar:${item.id}`, label: item.label, icon, items: item.menu };
		}
		return {
			type: 'action',
			id: item.command ?? item.id,
			label: item.label,
			icon,
			...(item.enabled ? {} : { disabled: true }),
			...(item.reason ? { title: item.reason } : {}),
		};
	});
}
