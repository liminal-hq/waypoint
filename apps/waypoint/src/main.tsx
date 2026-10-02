// Entry point: picks the screen for this webview's window kind
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { lazy, StrictMode, Suspense } from 'react';
import { createRoot } from 'react-dom/client';
import { AppWindowChrome } from './app/AppWindowChrome';
import { MainScreen } from './app/MainScreen';
import { OpsScreen } from './app/OpsScreen';
import { PropertiesScreen } from './app/PropertiesScreen';
import { ShelfScreen } from './app/ShelfScreen';
import { TearGhostScreen } from './app/TearGhostScreen';
import { windowKindFromLabel } from './app/windowKind';
import { MainSettings } from './settings/MainSettings';
import { initLogger } from './services/logger';
import { applyPlatform } from './theme/platform';
import { ThemeRoot } from './theme/ThemeRoot';
import type { WindowKind } from '@liminal-hq/waypoint-protocol/generated/WindowKind';
import './theme/tokens.css';

// The Settings window's pages are loaded when that window opens, so the main windows do not carry them.
const SettingsScreen = lazy(() =>
	import('./app/SettingsScreen').then((m) => ({ default: m.SettingsScreen })),
);

// A plain switch on the window kind: each webview hosts exactly one screen,
// so there is no in-window navigation to route yet.
function screenFor(kind: WindowKind | null) {
	switch (kind) {
		case 'Main':
			return (
				<MainSettings>
					<MainScreen />
				</MainSettings>
			);
		case 'Settings':
			return (
				<Suspense fallback={null}>
					<SettingsScreen />
				</Suspense>
			);
		case 'Properties':
			return <PropertiesScreen />;
		case 'Ops':
			return <OpsScreen />;
		case 'Shelf':
			return (
				<MainSettings>
					<ShelfScreen />
				</MainSettings>
			);
		case 'TearGhost':
			return <TearGhostScreen />;
		case null:
			return null;
	}
}

const label = getCurrentWebviewWindow().label;

initLogger(label);
applyPlatform();

createRoot(document.getElementById('root')!).render(
	<StrictMode>
		<ThemeRoot>
			<AppWindowChrome>{screenFor(windowKindFromLabel(label))}</AppWindowChrome>
		</ThemeRoot>
	</StrictMode>,
);
