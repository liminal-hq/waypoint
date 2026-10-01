// Gives a Main window the settings, and the shortcut that opens the Settings window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useState, type ReactNode } from 'react';
import type { SettingsClient } from '../services/settingsClient';
import { createTauriSettingsClient } from '../services/tauriSettingsClient';
import { useSettingsShortcut } from './openSettingsWindow';
import { SettingsProvider } from './SettingsContext';

interface MainSettingsProps {
	/** The plugin's client; the real one unless a test supplies its own. */
	client?: SettingsClient;
	children: ReactNode;
}

/**
 * Each window follows the settings itself: they arrive as events from Rust, the one owner, and
 * until the first answer a window uses the defaults, so nothing waits on them.
 */
export function MainSettings({ client, children }: MainSettingsProps) {
	const [own] = useState(() => client ?? createTauriSettingsClient());
	useSettingsShortcut();
	return <SettingsProvider client={own}>{children}</SettingsProvider>;
}
