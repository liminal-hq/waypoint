// Publishes the window's own facts and actions (Always on Top, Close Window, Settings) to the command bridge
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useWindowControls } from '@liminal-hq/waypoint-chrome/WindowChromeProvider/WindowChromeProvider';
import { useEffect, useState } from 'react';
import { useCommandBridge } from '../commands/commandBridge';
import { openSettingsWindow } from '../settings/openSettingsWindow';
import { useWindowCapabilities } from './windowCapabilities';

/**
 * Renders nothing. The title bar's own Always on Top button keeps its own state, so this follows
 * the window manager the same way (subscribe first, read second) to show the menu's check mark.
 * Where the window manager cannot keep a window on top the fact says so and the command is hidden.
 */
export function WindowCommands() {
	const bridge = useCommandBridge();
	const controls = useWindowControls();
	const capabilities = useWindowCapabilities();
	const supported = capabilities?.alwaysOnTop === true;
	const [on, setOn] = useState(false);

	useEffect(() => {
		let active = true;
		let changed = false;
		const unsubscribe = controls.onAlwaysOnTopChange?.((value) => {
			if (!active) return;
			changed = true;
			setOn(value);
		});
		void Promise.resolve(unsubscribe?.ready).then(() => {
			if (!active || !controls.isAlwaysOnTop) return;
			Promise.resolve(controls.isAlwaysOnTop())
				.then((value) => {
					if (active && !changed) setOn(value);
				})
				.catch(() => {});
		});
		return () => {
			active = false;
			unsubscribe?.();
		};
	}, [controls]);

	useEffect(() => {
		bridge.patchFacts({ alwaysOnTop: { supported, on } });
	}, [bridge, supported, on]);

	useEffect(() => {
		bridge.patchActions({
			setAlwaysOnTop: (value) => {
				setOn(value);
				void controls.setAlwaysOnTop(value);
			},
			closeWindow: () => void controls.close(),
			openSettings: openSettingsWindow,
		});
	}, [bridge, controls]);

	return null;
}
