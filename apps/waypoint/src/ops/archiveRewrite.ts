// Changes inside an open archive that ask first: a delete or a rename rewrites the whole archive file, so the person is told that, and how large it is, before it runs (D170)
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { PlanPreview } from '@liminal-hq/waypoint-protocol/generated/PlanPreview';
import { formatSize } from '../browse/format';
import { t, tf, tn } from '../i18n/messages';
import type { JobRequest } from '../services/opsClient';
import { limitOf } from './archiveRequests';
import type { ConfirmSpec } from './fileCommands';
import { commandErrorText } from './opsNotices';
import type { OpsHandle } from './opsStore';
import { problemText } from './problemModel';

/** What the confirmation says is being changed. */
export type RewriteSubject =
	| { kind: 'delete'; names: string[]; count: number }
	| { kind: 'rename'; name: string; newName: string };

/** The name of the archive file in a preview. */
function archiveName(preview: PlanPreview): string {
	const display = preview.archive?.container.display ?? '';
	return display.split(/[\\/]/).filter(Boolean).pop() ?? display;
}

/** The question for adding files to an archive that already has the name chosen in Compress: which file, how large, and that its format stays. */
export function addConfirm(preview: PlanPreview, what: string): ConfirmSpec {
	return {
		title: t('compress.exists.title'),
		message: tf('compress.exists.message', {
			archive: `“${archiveName(preview)}”`,
			what,
			size: formatSize(preview.archive?.size ?? 0),
		}),
		confirmLabel: t('compress.exists.action'),
		danger: false,
	};
}

/** The question for a change that rewrites the archive: which file, how large, and that Undo brings the old one back. */
export function rewriteConfirm(preview: PlanPreview, subject: RewriteSubject): ConfirmSpec {
	const archive = `“${archiveName(preview)}”`;
	const size = formatSize(preview.archive?.size ?? 0);
	if (subject.kind === 'rename') {
		return {
			title: t('archive.rewrite.rename.title'),
			message: tf('archive.rewrite.rename.message', {
				name: `“${subject.name}”`,
				newName: `“${subject.newName}”`,
				archive,
				size,
			}),
			confirmLabel: t('archive.rewrite.rename.action'),
			danger: false,
		};
	}
	const [only] = subject.names;
	return {
		title: t('archive.rewrite.delete.title'),
		message: tf('archive.rewrite.delete.message', {
			what: subject.count === 1 && only ? `“${only}”` : tn('dnd.items', subject.count),
			archive,
			size,
		}),
		items: subject.names,
		confirmLabel: t('archive.rewrite.delete.action'),
		danger: true,
	};
}

export interface RewriteDeps {
	ops: OpsHandle;
	confirm: (spec: ConfirmSpec) => Promise<boolean>;
	say: (text: string) => void;
}

/**
 * Plans a change to an archive and settles what planning finds before anything is written: a
 * refusal is said, an archive past the size limits is confirmed (and the request goes ahead with
 * `allowLarge`), and then the change itself is confirmed. Resolves to the request to run, or
 * `null` when it was refused or given up.
 */
export async function prepareRewrite(
	deps: RewriteDeps,
	request: JobRequest,
	confirm: (preview: PlanPreview) => ConfirmSpec,
): Promise<JobRequest | null> {
	let current = request;
	for (let attempt = 0; attempt < 2; attempt++) {
		let preview: PlanPreview;
		try {
			preview = await deps.ops.client.plan(current);
		} catch (failure) {
			const limit = limitOf(failure);
			if (limit && current.archive?.kind !== 'edit') {
				const reason = problemText(limit).details[0] ?? '';
				const name = limit.location.display.split(/[\\/]/).filter(Boolean).pop() ?? '';
				const ok = await deps.confirm({
					title: t('archive.rewrite.limit.title'),
					message: tf('archive.rewrite.limit.message', { name: `“${name}”`, reason }),
					confirmLabel: t('archive.rewrite.limit.action'),
					danger: true,
				});
				if (!ok) return null;
				current = { ...current, archive: { kind: 'edit', allowLarge: true } };
				continue;
			}
			deps.say(tf('files.failed', { reason: commandErrorText(failure) }));
			return null;
		}
		return (await deps.confirm(confirm(preview))) ? current : null;
	}
	return null;
}
