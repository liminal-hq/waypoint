// The toolbar button that shows and hides the sidebar
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { tf, t } from '../i18n/messages';
import { SidebarIcon } from '../icons/AppIcons';
import { NavButton } from '../nav/NavButton';
import { useSidebarState, useSidebarStore } from './sidebarStore';

/** A toggle button: it is pressed while the sidebar shows. */
export function SidebarToggle() {
	const store = useSidebarStore();
	const open = useSidebarState((state) => state.open);
	return (
		<NavButton
			label={t('sidebar.toggle')}
			title={tf('view.withShortcut', { name: t('sidebar.toggle'), keys: 'F9' })}
			pressed={open}
			icon={<SidebarIcon />}
			disabled={false}
			onPress={() => store.getState().toggleOpen()}
		/>
	);
}
