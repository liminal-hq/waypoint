// Which archive entries an extraction leaves out because of their stored names, read from the entry's attributes
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import { t } from '../i18n/messages';

/** The attribute the archive provider puts on an entry whose stored name is unsafe (`waypoint-provider-archive`). */
export const UNSAFE_NAME_ATTRIBUTE = 'archive.unsafe';

/** Words for a row that an extraction leaves out because of its name, or `null` for every other entry. */
export function leftOutNote(entry: Entry): string | null {
	return entry.attributes?.[UNSAFE_NAME_ATTRIBUTE] ? t('browse.archive.leftOut') : null;
}
