// The checksum of a file, on request: choose an algorithm, calculate, watch it, cancel it, copy the digest
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { EntryDetails } from '@liminal-hq/waypoint-protocol/generated/EntryDetails';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import type { VerifyAlgorithm } from '@liminal-hq/waypoint-protocol/generated/VerifyAlgorithm';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import { useEffect, useId, useRef, useState } from 'react';
import { formatSize } from '../browse/format';
import { t, tf, type MessageId } from '../i18n/messages';
import { CHECKSUM_ALGORITHMS, type ChecksumClient } from '../services/checksumClient';
import {
	algorithmMessage,
	canChecksum,
	DEFAULT_ALGORITHM,
	fraction,
	needsSizeWarning,
} from './checksumModel';
import { useChecksum } from './useChecksum';
import styles from './ChecksumSection.module.css';

/** How long "Copied" stays on the button. */
const COPIED_MS = 2000;

interface ChecksumSectionProps {
	client: ChecksumClient | null;
	handle: ListingHandle;
	entry: Entry;
	/** What the Inspector's facts say; the entry's own kind stands in until they are read. */
	details: EntryDetails | null;
	/** Puts text on the clipboard; the browser's unless a test supplies its own. */
	writeText?: (text: string) => Promise<void>;
}

function reasonMessage(error: VfsError): MessageId {
	switch (error.kind) {
		case 'isADirectory':
			return 'checksum.reason.directory';
		case 'unsupported':
			return 'checksum.reason.unsupported';
		case 'notFound':
			return 'checksum.reason.missing';
		case 'permissionDenied':
			return 'checksum.reason.denied';
		default:
			return 'checksum.reason.other';
	}
}

/**
 * Never automatic and never for a folder, a pipe or a device: nothing is read until the button is
 * pressed, and a file over 1 GiB asks first. The run is cancelled when the file changes under it or
 * the window closes. Only the finished digest is announced; progress would talk over the person.
 */
export function ChecksumSection({
	client,
	handle,
	entry,
	details,
	writeText = (text) => navigator.clipboard.writeText(text),
}: ChecksumSectionProps) {
	const kind = details?.kind ?? entry.kind;
	const resolvesTo = details ? details.resolvesTo : entry.linkTarget;
	const size = details?.size ?? entry.size;
	// The listing's own facts, not the details read a moment later: those arriving is not a change to the file.
	const subjectKey = `${handle}:${entry.id}:${entry.name}:${entry.modifiedMs}:${entry.size}`;
	const { state, start, cancel, reset } = useChecksum(client, handle, entry.id, subjectKey);
	const [algorithm, setAlgorithm] = useState<VerifyAlgorithm>(DEFAULT_ALGORITHM);
	const [confirming, setConfirming] = useState(false);
	const [copied, setCopied] = useState(false);
	const copyTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
	const heading = useId();
	const selectId = useId();

	// A new subject (or a changed file) starts over; a pending warning is for the file it was shown for.
	useEffect(() => {
		setConfirming(false);
		setCopied(false);
	}, [subjectKey]);
	useEffect(() => () => clearTimeout(copyTimer.current), []);

	if (!client || !canChecksum(kind, resolvesTo)) return null;

	const calculate = () => {
		if (state.status !== 'running' && needsSizeWarning(size) && !confirming) {
			setConfirming(true);
			return;
		}
		setConfirming(false);
		setCopied(false);
		start(algorithm);
	};
	const copy = (digest: string) => {
		void writeText(digest).then(() => {
			setCopied(true);
			clearTimeout(copyTimer.current);
			copyTimer.current = setTimeout(() => setCopied(false), COPIED_MS);
		});
	};

	const running = state.status === 'running';
	const announcement =
		state.status === 'done'
			? tf('checksum.announce.done', {
					algorithm: t(algorithmMessage(state.algorithm)),
					name: entry.name,
				})
			: '';

	return (
		<section className={styles.section} aria-labelledby={heading}>
			<h2 id={heading} className={styles.heading}>
				{t('checksum.title')}
			</h2>
			<p className={styles.note}>{t('checksum.note')}</p>
			<div className={styles.controls}>
				<label htmlFor={selectId} className={styles.label}>
					{t('checksum.algorithm.label')}
				</label>
				<select
					id={selectId}
					className={styles.select}
					value={algorithm}
					disabled={running}
					onChange={(event) => {
						setAlgorithm(event.target.value as VerifyAlgorithm);
						setConfirming(false);
						// A digest is of one algorithm; the other one's is not what is on screen.
						if (state.status !== 'idle') reset();
					}}
				>
					{CHECKSUM_ALGORITHMS.map((which) => (
						<option key={which} value={which}>
							{t(algorithmMessage(which))}
						</option>
					))}
				</select>
				{!running && !confirming && (
					<button type="button" className={styles.button} onClick={calculate}>
						{state.status === 'done' ? t('checksum.again') : t('checksum.calculate')}
					</button>
				)}
			</div>
			{confirming && (
				<div className={styles.warning}>
					<p className={styles.warningText}>
						{tf('checksum.largeWarning', { name: entry.name, size: formatSize(size ?? 0) })}
					</p>
					<div className={styles.actions}>
						<button type="button" className={styles.button} onClick={calculate}>
							{t('checksum.largeConfirm')}
						</button>
						<button type="button" className={styles.button} onClick={() => setConfirming(false)}>
							{t('checksum.cancel')}
						</button>
					</div>
				</div>
			)}
			{state.status === 'running' && (
				<div className={styles.progress}>
					<p className={styles.status}>
						{tf('checksum.running', { algorithm: t(algorithmMessage(state.algorithm)) })}
					</p>
					<progress
						className={styles.bar}
						aria-label={t('checksum.progress.label')}
						max={1}
						{...(state.total > 0 ? { value: fraction(state.bytesRead, state.total) } : {})}
					/>
					{state.total > 0 && (
						<p className={styles.detail}>
							{tf('checksum.progress.value', {
								read: formatSize(state.bytesRead),
								total: formatSize(state.total),
							})}
						</p>
					)}
					<div className={styles.actions}>
						<button type="button" className={styles.button} onClick={cancel}>
							{t('checksum.cancel')}
						</button>
					</div>
				</div>
			)}
			{state.status === 'done' && (
				<div className={styles.result}>
					<p className={styles.label}>
						{tf('checksum.result.label', { algorithm: t(algorithmMessage(state.algorithm)) })}
					</p>
					<code className={styles.digest} data-selectable="">
						{state.digest}
					</code>
					<div className={styles.actions}>
						<button type="button" className={styles.button} onClick={() => copy(state.digest)}>
							{copied ? t('checksum.copied') : t('checksum.copy')}
						</button>
					</div>
				</div>
			)}
			{state.status === 'cancelled' && <p className={styles.status}>{t('checksum.cancelled')}</p>}
			{state.status === 'failed' && (
				<p className={styles.status}>
					{tf('checksum.failed', { reason: t(reasonMessage(state.error)) })}
				</p>
			)}
			<div className={styles.srOnly} role="status" aria-live="polite">
				{announcement}
			</div>
		</section>
	);
}
