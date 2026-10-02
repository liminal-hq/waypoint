// The Integrations page: the Services panel now, and the integrations' own switches as each one is built
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { SettingsSection } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsSection';
import { ServicesPanel } from './ServicesPanel';

export function IntegrationsPage() {
	return (
		<SettingsSection>
			<ServicesPanel />
		</SettingsSection>
	);
}
