// Placeholder Properties window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t } from '../i18n/messages';
import { PlaceholderScreen } from './PlaceholderScreen';

export function PropertiesScreen() {
	return (
		<PlaceholderScreen
			title={t('window.properties.title')}
			description={t('window.properties.description')}
		/>
	);
}
