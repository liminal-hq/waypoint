// The Canadian French (fr-CA) catalogue, written from translations/fr-CA.json by `bun run i18n:import`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Catalogue } from '../active';

/**
 * Generated: edit `translations/fr-CA.json` (or use Weblate) and run `bun run i18n:import`. A
 * message missing here shows in English. Each `// source:` comment records the hash of the English
 * text the translation was made from, so `bun run i18n:status` can tell when English has moved on.
 */
const messages: Catalogue = {
	// source: c910d474
	'settings.section.general': 'Général',
	// source: 3907fa7f
	'settings.section.appearance': 'Apparence',
	// source: d3368cbf
	'settings.section.accessibility': 'Accessibilité',
	// source: 003e8754
	'settings.section.language': 'Langue et région',
	// source: e71a1345
	'settings.loading': 'Chargement des paramètres…',
	// source: 219e92dc
	'tabs.count.one': '{count} onglet',
	// source: ba82764a
	'tabs.count.other': '{count} onglets',
};

export default messages;
