// The app's title bar: the shared chrome configured for the current platform
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { TitleBar } from '@liminal-hq/waypoint-chrome/TitleBar';
import { TitleBarTitle } from '@liminal-hq/waypoint-chrome/TitleBar/TitleBarTitle';
import { fallbackTitlebarConfig } from './titlebarConfig';

interface AppTitleBarProps {
	title: string;
}

export function AppTitleBar({ title }: AppTitleBarProps) {
	const config = fallbackTitlebarConfig(document.documentElement.dataset.platform);
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
