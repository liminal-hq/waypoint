// Placeholder Ops window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t } from '../i18n/messages';
import { PlaceholderScreen } from './PlaceholderScreen';

export function OpsScreen() {
	return (
		<PlaceholderScreen title={t('window.ops.title')} description={t('window.ops.description')} />
	);
}
