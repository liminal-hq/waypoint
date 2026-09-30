// The app's title bar: the shared chrome configured from the OS's own preferences
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { TitleBar } from '@liminal-hq/waypoint-chrome/TitleBar';
import { TitleBarTitle } from '@liminal-hq/waypoint-chrome/TitleBar/TitleBarTitle';
import { useTitlebarPreferences } from '../services/titlebarPreferences';
import { titlebarConfigFor } from './titlebarConfig';

interface AppTitleBarProps {
	title: string;
}

export function AppTitleBar({ title }: AppTitleBarProps) {
	const preferences = useTitlebarPreferences();
	const config = titlebarConfigFor(preferences, document.documentElement.dataset.platform);
	return (
		<TitleBar
			center={<TitleBarTitle>{title}</TitleBarTitle>}
			controlsStyle={config.controlsStyle}
			buttonLayout={config.buttonLayout}
			titlebarActions={config.titlebarActions}
			titleAlign={config.titleAlign}
			showAlwaysOnTop
			transparent
		/>
	);
}
