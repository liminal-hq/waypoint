// Placeholder Main window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t } from '../i18n/messages';
import { PlaceholderScreen } from './PlaceholderScreen';

export function MainScreen() {
	return (
		<PlaceholderScreen title={t('window.main.title')} description={t('window.main.description')} />
	);
}
