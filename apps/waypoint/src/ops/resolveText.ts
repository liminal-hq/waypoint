// Words shared by the conflict and error dialogs: plural forms with extra values, choice labels and the summary of an answer
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ConflictPolicy } from '@liminal-hq/waypoint-protocol/generated/ConflictPolicy';
import { t, tf, type MessageId } from '../i18n/messages';
import { POLICY_ORDER, type Coverage } from './conflictModel';

/**
 * The `.one` or `.other` form of `base` for `count`, with `{count}` formatted for the locale and
 * the other `values` filled in (`tn` fills in the count only).
 */
export function pluralText(
	base: string,
	count: number,
	values: Record<string, string | number> = {},
	locale?: string,
): string {
	const form = new Intl.PluralRules(locale).select(count) === 'one' ? 'one' : 'other';
	return tf(`${base}.${form}` as MessageId, {
		...values,
		count: new Intl.NumberFormat(locale).format(count),
	});
}

export function policyLabel(policy: ConflictPolicy): string {
	return t(`ops.conflict.choice.${policy}` as MessageId);
}

/** "Skip 3, Keep both 1": what the answer comes to, for the live region. */
export function summaryText(counts: Coverage['byPolicy']): string {
	return POLICY_ORDER.filter((policy) => counts[policy] > 0)
		.map((policy) => tf(`ops.conflict.summary.${policy}` as MessageId, { count: counts[policy] }))
		.join(', ');
}
