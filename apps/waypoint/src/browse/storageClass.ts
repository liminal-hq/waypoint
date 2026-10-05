// The Storage class column's words: an object's storage class from the entry's attributes, and what it is called
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import { t, type MessageId } from '../i18n/messages';

/** The attribute the S3 provider puts the storage class under (`waypoint-provider-s3`). */
export const STORAGE_CLASS_ATTRIBUTE = 's3.storageClass';

const KNOWN: Record<string, MessageId> = {
	STANDARD: 'browse.storageClass.standard',
	STANDARD_IA: 'browse.storageClass.standardIa',
	ONEZONE_IA: 'browse.storageClass.onezoneIa',
	INTELLIGENT_TIERING: 'browse.storageClass.intelligentTiering',
	REDUCED_REDUNDANCY: 'browse.storageClass.reducedRedundancy',
	GLACIER_IR: 'browse.storageClass.glacierIr',
	GLACIER: 'browse.storageClass.glacier',
	DEEP_ARCHIVE: 'browse.storageClass.deepArchive',
	EXPRESS_ONEZONE: 'browse.storageClass.expressOnezone',
};

/** The classes whose objects must be restored before they can be read. */
const NEEDS_RESTORE: ReadonlySet<string> = new Set(['GLACIER', 'DEEP_ARCHIVE']);

/** Whether a folder's entries can carry a storage class: only a bucket on S3 does. */
export function hasStorageClasses(locationUri: string): boolean {
	return locationUri.startsWith('s3://');
}

/** The class the service reported for `entry`, as it spelled it, or `null` for a folder or a store that says none. */
export function storageClassOf(entry: Entry): string | null {
	return entry.attributes?.[STORAGE_CLASS_ATTRIBUTE] ?? null;
}

/** A class in words: the catalogue's name for a known one, the service's own spelling for another. */
export function storageClassLabel(name: string): string {
	const id = KNOWN[name];
	return id ? t(id) : name;
}

export function storageClassNeedsRestore(name: string): boolean {
	return NEEDS_RESTORE.has(name);
}
