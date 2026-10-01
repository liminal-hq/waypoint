// Default English labels and the context that carries them to the settings rows
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useContext } from 'react';

export interface SettingsLabels {
	/** Accessible name of the section navigation. */
	navigation: string;
	/** Prefix of the reason line on an unavailable row, e.g. "Unavailable: no portal found". */
	unavailable: string;
}

export const defaultSettingsLabels: SettingsLabels = {
	navigation: 'Settings sections',
	unavailable: 'Unavailable',
};

export const SettingsLabelsContext = createContext<SettingsLabels>(defaultSettingsLabels);

export function useSettingsLabels(): SettingsLabels {
	return useContext(SettingsLabelsContext);
}
