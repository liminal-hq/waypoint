// The optional menu bar under a Main window's title bar: the application menu's menus, laid out across it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { MenuBar } from '@liminal-hq/waypoint-chrome/MenuBar/MenuBar';
import { APP_MENU_MNEMONICS } from '../commands/appMenuModel';
import { t } from '../i18n/messages';
import { useAppMenu } from './useAppMenu';

/** The same menus, rows and commands as the title bar's application menu (Settings → General → Title bar). */
export function AppMenuBar() {
	const { items, onSelect } = useAppMenu();
	return (
		<MenuBar
			items={items}
			onSelect={onSelect}
			label={t('menuBar.label')}
			moreLabel={t('menuBar.more')}
			mnemonics={APP_MENU_MNEMONICS}
			transparent
		/>
	);
}
