// The words of a group header: its translated heading and the item count beside it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { GroupKey } from '@liminal-hq/waypoint-protocol/generated/GroupKey';
import { t, tf, tn, type MessageId } from '../i18n/messages';

/** The heading a group's key stands for, in the active language. */
export function groupTitle(key: GroupKey): string {
	switch (key.kind) {
		case 'kind':
			return t(`browse.header.kind.${key.group}`);
		case 'modified':
			return t(`browse.header.modified.${key.bucket}`);
		case 'year':
			return tf('browse.header.year', { year: key.year });
		case 'size':
			return t(`browse.header.size.${key.band}`);
		case 'name':
			return key.initial === '#' ? t('browse.header.name.symbols') : key.initial;
		case 'type':
			return key.extension === ''
				? t('browse.header.type.none')
				: tf('browse.header.type.extension', { extension: key.extension.toUpperCase() });
	}
}

/** The count of entries a header shows, as words ("3 items"). */
export function groupCount(count: number): string {
	return tn('browse.header.count', count);
}

/** What a screen reader says for a header: heading, count and whether it is open. */
export function groupLabel(key: GroupKey, count: number, collapsed: boolean): string {
	const id: MessageId = collapsed
		? 'browse.header.label.collapsed'
		: 'browse.header.label.expanded';
	return tf(id, { group: groupTitle(key), count: groupCount(count) });
}
