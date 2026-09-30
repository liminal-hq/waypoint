// Placeholder TearGhost window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t } from '../i18n/messages';
import { PlaceholderScreen } from './PlaceholderScreen';

export function TearGhostScreen() {
	return (
		<PlaceholderScreen
			title={t('window.tearGhost.title')}
			description={t('window.tearGhost.description')}
		/>
	);
}
