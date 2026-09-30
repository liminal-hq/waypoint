// Entry point: picks the screen for this webview's window kind
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { MainScreen } from './app/MainScreen';
import { OpsScreen } from './app/OpsScreen';
import { PropertiesScreen } from './app/PropertiesScreen';
import { SettingsScreen } from './app/SettingsScreen';
import { TearGhostScreen } from './app/TearGhostScreen';
import { windowKindFromLabel } from './app/windowKind';
import type { WindowKind } from './domain/protocol/generated/WindowKind';
import './theme/tokens.css';

// A plain switch on the window kind: each webview hosts exactly one screen,
// so there is no in-window navigation to route yet.
function screenFor(kind: WindowKind | null) {
	switch (kind) {
		case 'Main':
			return <MainScreen />;
		case 'Settings':
			return <SettingsScreen />;
		case 'Properties':
			return <PropertiesScreen />;
		case 'Ops':
			return <OpsScreen />;
		case 'TearGhost':
			return <TearGhostScreen />;
		case null:
			return null;
	}
}

const label = getCurrentWebviewWindow().label;

createRoot(document.getElementById('root')!).render(
	<StrictMode>{screenFor(windowKindFromLabel(label))}</StrictMode>,
);
