// The Open With submenu: the default application, the recommended ones, and Other Application…
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { App, Handlers } from '@liminal-hq/plugin-mime-apps';
import type { MenuItem, SubmenuMenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { PluginStatus } from '@liminal-hq/plugin-mime-apps';
import { t, tf } from '../i18n/messages';
import { openWithAbilities } from './OpenWithContext';
import { AppIcon, OpenWithIcon } from './OpenWithIcons';
import type { OpenWithChoice } from './startOpenWith';

/** The ids of the submenu's rows: the application's id follows `APP_PREFIX`. */
export const OPEN_WITH_IDS = {
	menu: 'openWith',
	default: 'openWith:default',
	other: 'openWith:other',
	chooser: 'openWith:chooser',
	loading: 'openWith:loading',
} as const;
const APP_PREFIX = 'openWith:app:';

/** What choosing a row of the submenu does. */
export type OpenWithAction = OpenWithChoice | { kind: 'other' };

/** What a row's id stands for; `null` for an id that is not one of the submenu's. */
export function openWithAction(id: string, handlers: Handlers | null): OpenWithAction | null {
	if (id === OPEN_WITH_IDS.default) return { kind: 'default' };
	if (id === OPEN_WITH_IDS.chooser) return { kind: 'chooser' };
	if (id === OPEN_WITH_IDS.other) return { kind: 'other' };
	if (!id.startsWith(APP_PREFIX) || !handlers) return null;
	const appId = id.slice(APP_PREFIX.length);
	const app = [handlers.default, ...handlers.recommended].find((each) => each?.id === appId);
	return app ? { kind: 'app', app } : null;
}

interface MenuInput {
	status: PluginStatus | null;
	/** The applications for the locations; `null` while they are being read. */
	handlers: Handlers | null;
	/** How many locations; the system's chooser takes one. */
	count: number;
	iconUrl: (appId: string) => string;
}

/**
 * The submenu, or `null` when Open With is not offered: the plugin cannot do it here, the
 * locations are of more than one type (the applications that open them all are not a menu a person
 * would expect), or no application is known for them. While `handlers` is `null` the submenu holds
 * a single disabled row, so the menu does not change shape when they arrive.
 */
export function openWithMenu({
	status,
	handlers,
	count,
	iconUrl,
}: MenuInput): SubmenuMenuItem | null {
	const can = openWithAbilities(status);
	if (!can.list && !can.openDefault && !can.chooser) return null;
	const menu = (items: MenuItem[]): SubmenuMenuItem => ({
		type: 'submenu',
		id: OPEN_WITH_IDS.menu,
		label: t('menu.openWith'),
		icon: <OpenWithIcon />,
		items,
	});
	if (!handlers) {
		return menu([
			{
				type: 'action',
				id: OPEN_WITH_IDS.loading,
				label: t('openWith.loading'),
				icon: <OpenWithIcon />,
				disabled: true,
			},
		]);
	}
	if (handlers.mixed) return null;

	const appRow = (app: App, label: string): MenuItem => ({
		type: 'action',
		id: `${APP_PREFIX}${app.id}`,
		label,
		icon: <AppIcon src={iconUrl(app.id)} />,
	});
	const first: MenuItem[] = [];
	if (can.openDefault && handlers.default) {
		first.push({
			type: 'action',
			id: OPEN_WITH_IDS.default,
			label: tf('openWith.default', { name: handlers.default.name }),
			icon: <AppIcon src={iconUrl(handlers.default.id)} />,
		});
	}
	const recommended: MenuItem[] = can.list
		? handlers.recommended.map((app) => appRow(app, app.name))
		: [];
	// The system's chooser takes one local file; otherwise the window draws the list itself.
	const other: MenuItem[] =
		can.chooser && count === 1
			? [
					{
						type: 'action',
						id: OPEN_WITH_IDS.chooser,
						label: t('openWith.other'),
						icon: <OpenWithIcon />,
					},
				]
			: can.list && handlers.others.length > 0
				? [
						{
							type: 'action',
							id: OPEN_WITH_IDS.other,
							label: t('openWith.other'),
							icon: <OpenWithIcon />,
						},
					]
				: [];
	const groups = [first, recommended, other].filter((group) => group.length > 0);
	if (groups.length === 0) return null;
	return menu(
		groups.flatMap((group, index) =>
			index === 0 ? group : [{ type: 'separator' } as const, ...group],
		),
	);
}
