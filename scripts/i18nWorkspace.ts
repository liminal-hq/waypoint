// Where this repository keeps its catalogues: the locales that are translated and the ones still in progress
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { join } from 'node:path';
import { INCOMPLETE_LOCALES } from '../apps/waypoint/src/i18n/catalogueParity';
import {
	FALLBACK_LOCALE,
	LOCALES,
	pluralTagFor,
	type Locale,
} from '../apps/waypoint/src/i18n/locales';
import type { Workspace } from './i18nTooling';

/** The repository's own i18n directory and locale lists, read from the app's modules. */
export function repositoryWorkspace(): Workspace {
	return {
		dir: join(import.meta.dirname, '../apps/waypoint/src/i18n'),
		// The pseudo-locales are generated from English and are not translated.
		locales: LOCALES.filter((locale) => locale !== FALLBACK_LOCALE),
		incomplete: INCOMPLETE_LOCALES,
		source: FALLBACK_LOCALE,
		pluralTag: (locale) => pluralTagFor(locale as Locale),
	};
}
