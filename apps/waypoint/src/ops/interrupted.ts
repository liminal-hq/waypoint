// What the page does with a transfer a lost connection stopped in an earlier run: resume it, or discard it after asking
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ResumableRecord } from '@liminal-hq/waypoint-protocol/generated/ResumableRecord';
import { t, tf, tn } from '../i18n/messages';
import type { OpsClient } from '../services/opsClient';
import { announce } from '../tabs/announcer';
import type { ConfirmSpec } from './fileCommands';
import { commandErrorText } from './opsNotices';

type Show = (text: string) => unknown;
type Client = Pick<OpsClient, 'resumeInterrupted' | 'discardInterrupted'>;

/**
 * The question Discard asks: it names the transfer and every file whose partial copy goes, so the
 * person sees what is removed from the server and that the transfer cannot be resumed after.
 */
export function discardSpec(record: ResumableRecord): ConfirmSpec {
	return {
		title: tf('ops.interrupted.discard.title', { label: record.label }),
		message: tn('ops.interrupted.discard.message', record.points.length),
		items: record.points.map((point) => point.target.display),
		confirmLabel: t('ops.interrupted.discard'),
		danger: true,
	};
}

/** Runs the transfer again from where it stopped; says why when it cannot. */
export async function resumeInterrupted(
	client: Client,
	record: ResumableRecord,
	show: Show,
): Promise<boolean> {
	try {
		await client.resumeInterrupted(record.job, record.atMs);
		announce(tf('ops.interrupted.resumed', { label: record.label }));
		return true;
	} catch (error) {
		show(tf('ops.recovery.resumeFailed', { reason: commandErrorText(error) }));
		return false;
	}
}

/** Asks, then removes the transfer's partial files and stops offering it; says why when it cannot. */
export async function discardInterrupted(
	client: Client,
	record: ResumableRecord,
	confirm: (spec: ConfirmSpec) => Promise<boolean>,
	show: Show,
): Promise<boolean> {
	if (!(await confirm(discardSpec(record)))) return false;
	try {
		await client.discardInterrupted(record.job, record.atMs);
		announce(tf('ops.interrupted.discarded', { label: record.label }));
		return true;
	} catch (error) {
		show(tf('ops.interrupted.discardFailed', { reason: commandErrorText(error) }));
		return false;
	}
}
