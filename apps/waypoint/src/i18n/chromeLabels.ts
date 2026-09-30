// Supplies the shared chrome's labels from the message catalogue
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ChromeLabels } from '@liminal-hq/waypoint-chrome/labels';
import { t } from './messages';

/** The chrome ships English defaults; the app passes its own so they follow the catalogue. */
export function chromeLabels(): ChromeLabels {
	return {
		restore: t('chrome.restore'),
		maximise: t('chrome.maximise'),
		minimise: t('chrome.minimise'),
		move: t('chrome.move'),
		alwaysOnTop: t('chrome.alwaysOnTop'),
		systemWindowMenu: t('chrome.systemWindowMenu'),
		close: t('chrome.close'),
		windowMenu: t('chrome.windowMenu'),
		windowControls: t('chrome.windowControls'),
	};
}
