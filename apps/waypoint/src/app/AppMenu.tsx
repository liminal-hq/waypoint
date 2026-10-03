// The application menu in the title bar: File, Edit, View and Window over the window's commands
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { AppMenuButton } from '@liminal-hq/waypoint-chrome/TitleBar/AppMenuButton';
import { APP_MENU_MNEMONICS } from '../commands/appMenuModel';
import { t } from '../i18n/messages';
import { AppMarkIcon } from '../icons/AppIcons';
import { useAppMenu } from './useAppMenu';

interface AppMenuProps {
	/**
	 * The menu bar is showing and carries these menus: the mark and name stay as plain text that
	 * opens nothing, and the keys are the bar's.
	 */
	menuBar?: boolean;
}

/**
 * The menu button for a Main window's title bar start slot. Its rows are the registry's commands
 * resolved for the window now, so a row is hidden, disabled with its reason as the tooltip, or
 * checked exactly as the Action bar and the keys see it, and choosing one runs the registry's
 * command. F10 or a lone Alt toggles it, and Alt+F, E, V and W open it with the menu named by the
 * letter open.
 */
export function AppMenu({ menuBar = false }: AppMenuProps) {
	const { items, onSelect } = useAppMenu();
	return (
		<AppMenuButton
			label={t('app.name')}
			mark={<AppMarkIcon />}
			items={items}
			onSelect={onSelect}
			mnemonics={APP_MENU_MNEMONICS}
			interactive={!menuBar}
		/>
	);
}
