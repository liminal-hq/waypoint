// The Trash view's actions: restore, delete permanently and empty, with the questions they ask along the way
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ConfirmDialog } from '@liminal-hq/waypoint-chrome/Dialog/ConfirmDialog';
import { Dialog } from '@liminal-hq/waypoint-chrome/Dialog/Dialog';
import { DialogActions, DialogButton } from '@liminal-hq/waypoint-chrome/Dialog/DialogActions';
import type { Conflict } from '@liminal-hq/waypoint-protocol/generated/Conflict';
import type { ConflictPolicy } from '@liminal-hq/waypoint-protocol/generated/ConflictPolicy';
import type { JobId } from '@liminal-hq/waypoint-protocol/generated/JobId';
import type { JobKind } from '@liminal-hq/waypoint-protocol/generated/JobKind';
import type { JobRequest } from '@liminal-hq/waypoint-protocol/generated/JobRequest';
import type { JobSnapshot } from '@liminal-hq/waypoint-protocol/generated/JobSnapshot';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { SelectionSpec } from '@liminal-hq/waypoint-protocol/generated/SelectionSpec';
import type { OpsError } from '@liminal-hq/waypoint-protocol/generated/OpsError';
import {
	createContext,
	useCallback,
	useContext,
	useEffect,
	useMemo,
	useRef,
	useState,
} from 'react';
import type { ReactNode } from 'react';
import { selectedCount } from '../browse/selection';
import type { ListingSession } from '../browse/useListingSession';
import { toSelectionSpec } from '../status/useSelectionSummary';
import { t, tf, tn } from '../i18n/messages';
import type { TrashClient } from './trashClient';

/** What the Trash view can do, for its menus, its action bar and its keys. */
export interface TrashActions {
	/** Restores the selected items to where they were. */
	restore(session: ListingSession): void;
	/** Asks, then deletes the selected items for good. */
	deletePermanently(session: ListingSession): void;
	/** Asks, then empties the Trash. `count` is how many items it holds, for the question. */
	emptyTrash(count: number): void;
	/** Says what Enter and a double-click do in the Trash, which is nothing. */
	hintOpen(): void;
}

const TrashActionsContext = createContext<TrashActions | null>(null);

export const TrashActionsProvider = TrashActionsContext.Provider;

/** The window's Trash actions, or `null` where there is no Trash service. */
export function useTrashActions(): TrashActions | null {
	return useContext(TrashActionsContext);
}

/** The words for why an operation failed, from the engine's typed error. */
export function opsErrorText(error: OpsError): string {
	switch (error.kind) {
		case 'notFound':
		case 'permissionDenied':
		case 'nameInUse':
		case 'cannotReplace':
			return tf(`trash.reason.${error.kind}`, { location: error.location.display });
		case 'trashUnavailable':
			return error.reason;
		case 'originMissingParent':
			return tf('trash.parent.message', { folder: error.location.display });
		case 'io':
		case 'unsupported':
			return error.kind === 'io' ? error.message : error.what;
		default:
			return t('trash.reason.generic');
	}
}

/** Why a command was refused: the plugin's message when it sent one. */
function rejectionText(error: unknown): string {
	if (typeof error === 'object' && error !== null && 'message' in error) {
		const { message } = error as { message: unknown };
		if (typeof message === 'string' && message !== '') return message;
	}
	return typeof error === 'string' && error !== '' ? error : t('trash.reason.generic');
}

type Verb = 'restore' | 'delete' | 'empty';

/**
 * The ids a selection names, written out when a question is put. A select-all is "everything
 * except these", which the engine would resolve against the listing as it is when the job plans:
 * items that another program trashed in the meantime would be deleted for good beyond the count the
 * person confirmed. Written out, the job holds exactly the items the question counted.
 */
async function pinSelection(session: ListingSession): Promise<SelectionSpec> {
	const selection = session.store.getState().selection;
	if (selection.kind === 'some') return { kind: 'some', ids: [...selection.ids] };
	const entries = await session.model.readRange(0, session.model.count);
	return {
		kind: 'some',
		ids: entries.map((entry) => entry.id).filter((id) => !selection.ids.has(id)),
	};
}

/** What a job was started to do, kept until it ends. */
interface Tracked {
	verb: Verb;
}

type Question =
	| { kind: 'delete'; session: ListingSession; count: number; spec: SelectionSpec }
	| { kind: 'empty'; count: number }
	| { kind: 'conflicts'; job: JobId; conflicts: Conflict[] }
	| { kind: 'parent'; job: JobId; folder: Location }
	| { kind: 'problem'; job: JobId; text: string };

function request(kind: JobKind, sources: JobRequest['sources']): JobRequest {
	return {
		kind,
		sources,
		destination: null,
		name: null,
		options: { conflict: null, verify: null },
		// The operations plugin fills in the calling window.
		originWindow: '',
	};
}

/**
 * Runs the Trash view's jobs through `client` and follows them: it asks before anything is deleted
 * for good, and when a restore parks on a question (a name that is taken, a folder that is gone) it
 * puts the question to the person and passes the answer on. Only jobs this window started are
 * followed, so each question is asked once, in the window that asked for the job. What happened is
 * reported through `notify`.
 *
 * Returns the actions and the dialogs they use; render the dialogs anywhere in the window.
 */
export function useTrashJobs(
	client: TrashClient | null,
	notify: (text: string) => void,
): { actions: TrashActions | null; dialogs: ReactNode } {
	const [question, setQuestion] = useState<Question | null>(null);
	const tracked = useRef(new Map<JobId, Tracked>());
	// Events can arrive before `submit` resolves with the job's id, so the last of each is kept.
	const latest = useRef(new Map<JobId, JobSnapshot>());
	const notifyRef = useRef(notify);
	notifyRef.current = notify;

	const follow = useCallback((job: JobSnapshot) => {
		const mine = tracked.current.get(job.id);
		if (!mine) return;
		const say = (text: string) => notifyRef.current(text);
		const { state } = job;
		// Whatever the job was waiting on is answered or moot once it moves on.
		if (state.state !== 'waiting') {
			setQuestion((now) => (now && 'job' in now && now.job === job.id ? null : now));
		}
		switch (state.state) {
			case 'done': {
				tracked.current.delete(job.id);
				const done = Number(job.progress.itemsDone);
				const skipped = Number(job.counts.skipped);
				if (mine.verb === 'empty') say(t('trash.done.emptied'));
				else {
					const base =
						mine.verb === 'restore'
							? tn('trash.done.restored', done)
							: tn('trash.done.deleted', done);
					say(skipped > 0 ? `${base} · ${tn('trash.done.skipped', skipped)}` : base);
				}
				return;
			}
			case 'failed':
				tracked.current.delete(job.id);
				say(tf(`trash.failed.${mine.verb}`, { reason: opsErrorText(state.error) }));
				return;
			case 'cancelled':
				tracked.current.delete(job.id);
				return;
			case 'waiting': {
				const { reason } = state;
				if (reason.kind === 'conflicts') {
					setQuestion({ kind: 'conflicts', job: job.id, conflicts: reason.conflicts });
				} else if (reason.error.kind === 'originMissingParent') {
					setQuestion({ kind: 'parent', job: job.id, folder: reason.error.location });
				} else {
					setQuestion({ kind: 'problem', job: job.id, text: opsErrorText(reason.error) });
				}
				return;
			}
			default:
				return;
		}
	}, []);

	useEffect(() => {
		if (!client) return;
		return client.onEvent((event) => {
			if (event.kind === 'jobAdded' || event.kind === 'jobChanged') {
				latest.current.set(event.job.id, event.job);
				follow(event.job);
			} else if (event.kind === 'jobRemoved') {
				latest.current.delete(event.id);
				tracked.current.delete(event.id);
			}
		});
	}, [client, follow]);

	const start = useCallback(
		(verb: Verb, job: JobRequest) => {
			if (!client) return;
			client.submit(job).then(
				(id) => {
					tracked.current.set(id, { verb });
					const seen = latest.current.get(id);
					if (seen) follow(seen);
				},
				(error: unknown) => {
					console.warn('could not start the Trash job', error);
					notifyRef.current(tf('trash.failed.submit', { reason: rejectionText(error) }));
				},
			);
		},
		[client, follow],
	);

	const actions = useMemo<TrashActions | null>(() => {
		if (!client) return null;
		const selection = (session: ListingSession) => {
			const state = session.store.getState().selection;
			return {
				count: selectedCount(state, session.model.count),
				sources: {
					kind: 'selection' as const,
					handle: session.model.handle,
					spec: toSelectionSpec(state),
				},
			};
		};
		return {
			restore(session) {
				const { count, sources } = selection(session);
				if (count > 0) start('restore', request({ kind: 'restore' }, sources));
			},
			deletePermanently(session) {
				if (selection(session).count === 0) return;
				pinSelection(session).then(
					(spec) => {
						if (spec.ids.length > 0) {
							setQuestion({ kind: 'delete', session, count: spec.ids.length, spec });
						}
					},
					(error: unknown) => {
						console.warn('could not read the selection', error);
						notifyRef.current(tf('trash.failed.submit', { reason: rejectionText(error) }));
					},
				);
			},
			emptyTrash(count) {
				if (count > 0) setQuestion({ kind: 'empty', count });
			},
			hintOpen() {
				notifyRef.current(t('trash.hint.open'));
			},
		};
	}, [client, start]);

	const answer = useCallback(
		(work: Promise<void>) =>
			work.catch((error: unknown) => {
				console.warn('could not answer the Trash job', error);
				notifyRef.current(tf('trash.failed.submit', { reason: rejectionText(error) }));
			}),
		[],
	);

	const dialogs = (
		<TrashDialogs
			question={question}
			onClose={() => setQuestion(null)}
			onConfirmDelete={() => {
				if (question?.kind !== 'delete') return;
				const { session, spec } = question;
				setQuestion(null);
				start(
					'delete',
					request({ kind: 'delete' }, { kind: 'selection', handle: session.model.handle, spec }),
				);
			}}
			onConfirmEmpty={() => {
				setQuestion(null);
				start(
					'empty',
					request(
						{ kind: 'emptyTrash', olderThanDays: null },
						{ kind: 'locations', locations: [] },
					),
				);
			}}
			onConflicts={(policy: ConflictPolicy) => {
				if (question?.kind !== 'conflicts' || !client) return;
				const { job } = question;
				setQuestion(null);
				void answer(client.resolveConflicts(job, policy));
			}}
			onStop={() => {
				if (!question || !('job' in question) || !client) return setQuestion(null);
				const { job } = question;
				setQuestion(null);
				void answer(client.cancel(job));
			}}
			onDecision={(decision) => {
				if (!question || !('job' in question) || !client) return;
				const { job } = question;
				setQuestion(null);
				void answer(client.resolveError(job, decision));
			}}
		/>
	);
	return { actions, dialogs };
}

interface TrashDialogsProps {
	question: Question | null;
	onClose(): void;
	onConfirmDelete(): void;
	onConfirmEmpty(): void;
	onConflicts(policy: ConflictPolicy): void;
	/** The question was dismissed: the job stops. */
	onStop(): void;
	onDecision(decision: 'createParents' | 'skip'): void;
}

function TrashDialogs({
	question,
	onClose,
	onConfirmDelete,
	onConfirmEmpty,
	onConflicts,
	onStop,
	onDecision,
}: TrashDialogsProps) {
	const cancel = t('trash.confirm.cancel');
	const conflicts = question?.kind === 'conflicts' ? question.conflicts : [];
	// Only a file may replace a file; any other clash can be kept beside or skipped.
	const canReplace = conflicts.length > 0 && conflicts.every((c) => c.kind === 'fileOverFile');
	return (
		<>
			<ConfirmDialog
				open={question?.kind === 'delete'}
				title={t('trash.confirm.delete.title')}
				message={
					question?.kind === 'delete' ? tn('trash.confirm.delete.message', question.count) : ''
				}
				confirmLabel={t('menu.deletePermanently')}
				cancelLabel={cancel}
				danger
				onConfirm={onConfirmDelete}
				onCancel={onClose}
			/>
			<ConfirmDialog
				open={question?.kind === 'empty'}
				title={t('trash.confirm.empty.title')}
				message={
					question?.kind === 'empty' ? tn('trash.confirm.empty.message', question.count) : ''
				}
				confirmLabel={t('menu.emptyTrash')}
				cancelLabel={cancel}
				danger
				onConfirm={onConfirmEmpty}
				onCancel={onClose}
			/>
			<Dialog
				open={question?.kind === 'conflicts'}
				title={t('trash.conflict.title')}
				description={
					conflicts.length === 1
						? tf('trash.conflict.message.one', { name: conflicts[0]?.name ?? '' })
						: tn('trash.conflict.message', conflicts.length)
				}
				size="small"
				onClose={onStop}
				footer={
					<DialogActions>
						<DialogButton variant="secondary" onClick={onStop}>
							{t('trash.conflict.cancel')}
						</DialogButton>
						<DialogButton variant="secondary" onClick={() => onConflicts('skip')}>
							{t('trash.conflict.skip')}
						</DialogButton>
						{canReplace && (
							<DialogButton variant="secondary" onClick={() => onConflicts('replace')}>
								{t('trash.conflict.replace')}
							</DialogButton>
						)}
						<DialogButton variant="primary" onClick={() => onConflicts('keepBoth')}>
							{t('trash.conflict.keepBoth')}
						</DialogButton>
					</DialogActions>
				}
			/>
			<Dialog
				open={question?.kind === 'parent'}
				title={t('trash.parent.title')}
				description={
					question?.kind === 'parent'
						? tf('trash.parent.message', { folder: question.folder.display })
						: ''
				}
				size="small"
				onClose={onStop}
				footer={
					<DialogActions>
						<DialogButton variant="secondary" onClick={onStop}>
							{t('trash.parent.cancel')}
						</DialogButton>
						<DialogButton variant="secondary" onClick={() => onDecision('skip')}>
							{t('trash.parent.skip')}
						</DialogButton>
						<DialogButton variant="primary" onClick={() => onDecision('createParents')}>
							{t('trash.parent.create')}
						</DialogButton>
					</DialogActions>
				}
			/>
			<Dialog
				open={question?.kind === 'problem'}
				title={t('trash.error.title')}
				description={question?.kind === 'problem' ? question.text : ''}
				size="small"
				onClose={onStop}
				footer={
					<DialogActions>
						<DialogButton variant="secondary" onClick={onStop}>
							{t('trash.error.cancel')}
						</DialogButton>
						<DialogButton variant="primary" onClick={() => onDecision('skip')}>
							{t('trash.error.skip')}
						</DialogButton>
					</DialogActions>
				}
			/>
		</>
	);
}
