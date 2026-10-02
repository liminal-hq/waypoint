// The Canadian French catalogue: a few messages while the loader is proved, the full translation is a later slice
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Catalogue } from '../active';

/**
 * Incomplete on purpose: a message missing here shows in English, and `catalogueParity` lists
 * `fr-CA` as incomplete until the translation lands (slice 22).
 */
const frMessages: Catalogue = {
	'settings.section.general': 'Général',
	'settings.section.appearance': 'Apparence',
	'settings.section.accessibility': 'Accessibilité',
	'settings.section.language': 'Langue et région',
	'settings.loading': 'Chargement des paramètres…',
	'tabs.count.one': '{count} onglet',
	'tabs.count.other': '{count} onglets',
};

export default frMessages;
