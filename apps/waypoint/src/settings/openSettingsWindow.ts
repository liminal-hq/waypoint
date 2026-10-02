// Opens the single Settings window, or brings it forward, and the shortcut that does it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import { useEffect } from 'react';
import { showNotice } from '../app/notices';
import { t } from '../i18n/messages';

/** Opens the Settings window, or focuses the one that is open. A failure is shown as a notice. */
export function openSettingsWindow(): void {
	invoke<void>('open_settings_window').catch((error: unknown) => {
		console.warn('could not open the Settings window', error);
		showNotice(t('settings.open.failed'));
	});
}

type KeyEventLike = Pick<
	KeyboardEvent,
	'key' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey' | 'isComposing'
>;

/** Whether the key is the Settings shortcut, Ctrl+, (Cmd+, on a Mac keyboard). */
export function isSettingsShortcut(event: KeyEventLike): boolean {
	return (
		!event.isComposing &&
		event.key === ',' &&
		(event.ctrlKey || event.metaKey) &&
		!event.altKey &&
		!event.shiftKey
	);
}

/**
 * Opens the Settings window on Ctrl+,. It works wherever focus is, including a text field, because
 * the key means nothing to a field; `open` is a parameter so a test can watch it.
 */
export function useSettingsShortcut(open: () => void = openSettingsWindow): void {
	useEffect(() => {
		const onKeyDown = (event: KeyboardEvent) => {
			if (event.defaultPrevented || !isSettingsShortcut(event)) return;
			event.preventDefault();
			open();
		};
		window.addEventListener('keydown', onKeyDown);
		return () => window.removeEventListener('keydown', onKeyDown);
	}, [open]);
}
