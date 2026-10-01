// Data model for the settings shell: the sections an app hands it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ReactNode } from 'react';

/** One page of settings. The app builds the list from data; the shell only renders it. */
export interface SettingsSectionDef {
	/** Stable identifier, used for selection and element ids. */
	id: string;
	/** Nav label and the content heading. */
	label: string;
	/** Optional 16 px icon shown beside the label in the side nav. */
	icon?: ReactNode;
	/** Renders the page body, normally one or more `SettingsGroup`s. Called only for the active section. */
	render: () => ReactNode;
}
