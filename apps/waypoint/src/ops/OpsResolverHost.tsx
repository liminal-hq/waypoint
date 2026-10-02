// Answers the jobs that wait for the person: opens the conflict or error dialog for the window's own jobs, and for any job when "Resolve…" is pressed
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { JobSnapshot } from '@liminal-hq/waypoint-protocol/generated/JobSnapshot';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useStore } from 'zustand';
import { showNotice } from '../app/notices';
import { t, tf } from '../i18n/messages';
import { announce } from '../tabs/announcer';
import { ConflictDialog, type ConflictAnswer } from './ConflictDialog';
import { conflictsKey } from './conflictModel';
import { useOps } from './OpsContext';
import type { OpsHandle } from './opsStore';
import { OperationErrorDialog } from './OperationErrorDialog';
import { registerResolver } from './resolveHook';
import { summaryText } from './resolveText';

/** What a wait is about, so the same wait is not opened twice and a new one is. */
function waitKey(job: JobSnapshot): string | null {
	const { state } = job;
	if (state.state !== 'waiting') return null;
	return state.reason.kind === 'conflicts'
		? `c:${conflictsKey(state.reason.conflicts)}`
		: `e:${state.reason.error.kind}:${state.reason.item.uri}`;
}

function failureText(error: unknown): string {
	if (error && typeof error === 'object' && 'message' in error) {
		return String((error as { message: unknown }).message);
	}
	return String(error);
}

/**
 * The one place a window answers a waiting job, mounted once inside the queue's provider. It
 * registers the resolver behind "Resolve…", so a row's button opens the right dialog for the
 * job's reason, and it opens the dialog by itself for the jobs this window started (their
 * `originWindow` is its label), so each question is asked in one window. Another window answering
 * first closes the dialog when the job leaves `Waiting`. "Decide later" parks a question until
 * "Resolve…" asks for it again.
 */
export function OpsResolverHost() {
	const ops = useOps();
	if (!ops) return null;
	return <Host handle={ops.handle} windowLabel={ops.windowLabel} />;
}

function Host({ handle, windowLabel }: { handle: OpsHandle; windowLabel: string }) {
	const snapshot = useStore(handle.store, (state) => state.snapshot);
	// The job a person asked for with "Resolve…", whoever started it.
	const [forced, setForced] = useState<number | null>(null);
	// Jobs whose current question is answered, sent or put off, by the wait they were for.
	const [parked, setParked] = useState<ReadonlyMap<number, string>>(new Map());

	const jobs = snapshot?.jobs ?? [];
	const waiting = jobs.filter((job) => job.state.state === 'waiting');
	const forcedJob = waiting.find((job) => job.id === forced);
	const target =
		forcedJob ??
		waiting.find((job) => job.originWindow === windowLabel && parked.get(job.id) !== waitKey(job));

	// A job that moved on needs no entry of its own any more.
	const waitingIds = waiting.map((job) => job.id).join(',');
	useEffect(() => {
		const live = new Set(waitingIds === '' ? [] : waitingIds.split(',').map(Number));
		setForced((now) => (now !== null && !live.has(now) ? null : now));
		setParked((now) => {
			if ([...now.keys()].every((id) => live.has(id))) return now;
			return new Map([...now].filter(([id]) => live.has(id)));
		});
	}, [waitingIds]);

	// A dialog gives focus back to the element that had it, but not when that element is gone (the
	// popover's "Resolve…" button closed with the popover): the operations button takes it then,
	// so focus never rests on the page body.
	const asking = target !== undefined && target.state.state === 'waiting';
	const wasAsking = useRef(false);
	useEffect(() => {
		if (wasAsking.current && !asking) {
			const active = document.activeElement;
			if (!active || active === document.body) {
				document.querySelector<HTMLElement>('[data-ops-ring]')?.focus();
			}
		}
		wasAsking.current = asking;
	}, [asking]);

	const handleRef = useRef(handle);
	handleRef.current = handle;
	useEffect(
		() =>
			registerResolver((id) => {
				const job = handleRef.current.store.getState().snapshot?.jobs.find((j) => j.id === id);
				if (job?.state.state === 'waiting') setForced(id);
				else showNotice(t('ops.resolve.notWaiting'));
			}),
		[],
	);

	const park = useCallback((job: JobSnapshot) => {
		const key = waitKey(job);
		setForced(null);
		if (key !== null) setParked((now) => new Map(now).set(job.id, key));
	}, []);
	const unpark = useCallback((job: JobSnapshot) => {
		setParked((now) => {
			const next = new Map(now);
			next.delete(job.id);
			return next;
		});
		setForced(job.id);
	}, []);
	const send = useCallback(
		(job: JobSnapshot, work: Promise<unknown>) => {
			work.catch((error: unknown) => {
				showNotice(tf('ops.resolve.failed', { reason: failureText(error) }));
				// Still waiting: ask again rather than leave the job stuck with no dialog.
				if (
					handleRef.current.store
						.getState()
						.snapshot?.jobs.some((j) => j.id === job.id && j.state.state === 'waiting')
				) {
					unpark(job);
				}
			});
		},
		[unpark],
	);

	if (!target || target.state.state !== 'waiting') return null;
	const { reason } = target.state;
	const key = `${target.id}:${waitKey(target)}`;
	const { client } = handle;

	if (reason.kind === 'conflicts') {
		return (
			<ConflictDialog
				key={key}
				job={target}
				conflicts={reason.conflicts}
				onContinue={(answer: ConflictAnswer) => {
					park(target);
					announce(tf('ops.conflict.announce', { summary: summaryText(answer.counts) }));
					send(target, client.resolve(target.id, answer.decisions, answer.applyToAll));
				}}
				onCancelJob={() => {
					park(target);
					send(target, client.cancel(target.id));
				}}
				onLater={() => park(target)}
			/>
		);
	}
	return (
		<OperationErrorDialog
			key={key}
			job={target}
			error={reason.error}
			item={reason.item}
			onDecide={(decision) => {
				park(target);
				send(target, client.resolveError(target.id, decision));
			}}
			onLater={() => park(target)}
		/>
	);
}
