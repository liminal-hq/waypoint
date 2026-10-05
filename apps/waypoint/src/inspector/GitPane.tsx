// The Git tab: the repository and branch, and for the selected item its status, what changed since the last commit and the commits that touched it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { GitPathInfo } from '@liminal-hq/waypoint-protocol/generated/GitPathInfo';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { useEffect, useState, type ReactNode } from 'react';
import { formatModified } from '../browse/format';
import { useHourCycle } from '../browse/TimeFormatContext';
import { useOptionalVfsClient } from '../browse/VfsClientContext';
import { useGitClient } from '../git/GitContext';
import { branchText, markWords, summaryCounts } from '../git/gitModel';
import type { Repository } from '../git/gitStore';
import { t, tf, tn, type MessageId } from '../i18n/messages';
import { HEAVY_DELAY_MS } from './inspectorModel';
import type { InspectorSubject } from './useInspectorSubject';
import { useSettled } from './useSettled';
import styles from './InspectorPanel.module.css';

type History =
	{ status: 'loading' } | { status: 'ready'; info: GitPathInfo } | { status: 'failed' };

/** The location whose history to read: the selected item's, or the folder shown when nothing is selected. */
function useTarget(subject: InspectorSubject): Location | null {
	const vfs = useOptionalVfsClient();
	const [resolved, setResolved] = useState<{ key: string; location: Location } | null>(null);
	const key =
		subject.kind === 'entry'
			? `entry:${subject.handle}:${subject.entry.id}`
			: subject.kind === 'folder'
				? `folder:${subject.location.uri}`
				: '';
	const settled = useSettled(key, HEAVY_DELAY_MS);
	useEffect(() => {
		if (!settled || subject.kind !== 'entry' || !vfs) return;
		let live = true;
		vfs.entryLocation(subject.handle, subject.entry.id).then(
			(location) => live && setResolved({ key, location }),
			(error: unknown) => console.warn('could not locate the selected item', error),
		);
		return () => {
			live = false;
		};
		// The subject is read through its key: a new object for the same entry is the same question.
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [settled, key, vfs]);
	if (!settled) return null;
	if (subject.kind === 'folder') return subject.location;
	return resolved?.key === key ? resolved.location : null;
}

/** Reads the history of `target` (after the selection has held still) and drops it when the subject moves on. */
function useHistory(target: Location | null): History {
	const client = useGitClient();
	const [result, setResult] = useState<{ uri: string; state: History } | null>(null);
	const uri = target?.uri ?? null;
	useEffect(() => {
		if (!client || !target || uri === null) return;
		let live = true;
		client.pathInfo(target, 5).then(
			(info) => {
				if (!live) return;
				setResult({ uri, state: info ? { status: 'ready', info } : { status: 'failed' } });
			},
			(error: unknown) => {
				console.warn('could not read the Git history', error);
				if (live) setResult({ uri, state: { status: 'failed' } });
			},
		);
		return () => {
			live = false;
		};
		// The uri names the target; a new object for the same one is the same question.
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [client, uri]);
	if (uri === null || result?.uri !== uri) return { status: 'loading' };
	return result.state;
}

function Row({ field, children }: { field: MessageId; children: ReactNode }) {
	return (
		<div className={styles.fact}>
			<dt>{t(field)}</dt>
			<dd data-selectable="">{children}</dd>
		</div>
	);
}

/** The tab's content for one repository and subject. Read only: nothing here changes the repository. */
export function GitPane({
	subject,
	repository,
}: {
	subject: InspectorSubject;
	repository: Repository;
}) {
	const hourCycle = useHourCycle();
	const target = useTarget(subject);
	const history = useHistory(target);
	const summary = repository.summary;
	const mark = subject.kind === 'entry' ? subject.entry.git : undefined;
	const counts = summary ? summaryCounts(summary) : '';

	return (
		<>
			<div className={styles.summary}>
				<p className={styles.summaryTitle} data-selectable="">
					{tf('git.pane.repositoryName', { name: repository.name })}
				</p>
				{summary && <p className={styles.summaryFacts}>{branchText(summary)}</p>}
			</div>
			{summary && (
				<dl className={styles.rows} aria-label={t('git.pane.repository')}>
					<Row field="git.pane.branch">{branchText(summary)}</Row>
					{summary.upstream && <Row field="git.pane.upstream">{summary.upstream}</Row>}
					<Row field="git.pane.changes">{counts === '' ? t('git.pane.clean') : counts}</Row>
					{summary.operation && (
						<Row field="git.pane.operation">
							{t(`git.operation.${summary.operation}` as MessageId)}
						</Row>
					)}
				</dl>
			)}
			{subject.kind === 'many' && <p className={styles.summaryFacts}>{t('git.pane.many')}</p>}
			{(subject.kind === 'entry' || subject.kind === 'folder') && (
				<section aria-label={t('git.pane.item')}>
					{subject.kind === 'entry' && (
						<dl className={styles.rows}>
							<Row field="git.pane.status">
								{mark && markWords(mark) !== '' ? markWords(mark) : t('git.pane.clean')}
							</Row>
						</dl>
					)}
					{history.status === 'loading' && (
						<p className={styles.summaryFacts} role="status">
							{t('git.pane.loading')}
						</p>
					)}
					{history.status === 'failed' && (
						<p className={styles.summaryFacts} role="status">
							{t('git.pane.failed')}
						</p>
					)}
					{history.status === 'ready' && <Details info={history.info} hourCycle={hourCycle} />}
				</section>
			)}
		</>
	);
}

function Details({
	info,
	hourCycle,
}: {
	info: GitPathInfo;
	hourCycle: ReturnType<typeof useHourCycle>;
}) {
	const { diff } = info;
	return (
		<>
			<h3 className={styles.summaryFacts}>{t('git.pane.sinceCommit')}</h3>
			<dl className={styles.rows}>
				{diff.files === 0 ? (
					<Row field="git.pane.changes">{t('git.pane.clean')}</Row>
				) : (
					<>
						<Row field="git.pane.changes">{tn('git.pane.diff.files', diff.files)}</Row>
						<Row field="git.pane.status">
							{tf('git.pane.diff.lines', { added: diff.added, removed: diff.removed })}
							{diff.binary > 0 && ` · ${tn('git.pane.diff.binary', diff.binary)}`}
						</Row>
					</>
				)}
			</dl>
			{diff.partial && <p className={styles.summaryFacts}>{t('git.pane.diff.partial')}</p>}
			<h3 className={styles.summaryFacts}>{t('git.pane.commits')}</h3>
			{info.commits.length === 0 ? (
				<p className={styles.summaryFacts}>{t('git.pane.noCommits')}</p>
			) : (
				<ol className={styles.rows} aria-label={t('git.pane.commits')}>
					{info.commits.map((commit) => (
						<li key={commit.id} className={styles.fact} data-selectable="">
							<span>
								<span>{commit.summary}</span>
								<br />
								<span className={styles.summaryFacts}>
									{commit.short} ·{' '}
									{tf('git.pane.commit.by', {
										author: commit.author,
										date: formatModified(commit.timeMs, undefined, hourCycle),
									})}
								</span>
							</span>
						</li>
					))}
				</ol>
			)}
			{info.truncated && <p className={styles.summaryFacts}>{t('git.pane.truncated')}</p>}
		</>
	);
}
