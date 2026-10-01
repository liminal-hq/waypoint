// The question an item that failed puts to the person: retry it, skip it, skip every failure like it, or cancel the job
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Dialog } from '@liminal-hq/waypoint-chrome/Dialog/Dialog';
import { DialogActions, DialogButton } from '@liminal-hq/waypoint-chrome/Dialog/DialogActions';
import type { Decision } from '@liminal-hq/waypoint-protocol/generated/Decision';
import type { JobSnapshot } from '@liminal-hq/waypoint-protocol/generated/JobSnapshot';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { OpsError } from '@liminal-hq/waypoint-protocol/generated/OpsError';
import { t, tf, type MessageId } from '../i18n/messages';
import { jobTitle } from './jobText';
import styles from './OperationErrorDialog.module.css';
import { decisionsFor, problemText } from './problemModel';

export interface OperationErrorDialogProps {
	job: JobSnapshot;
	error: OpsError;
	/** The item that failed. */
	item: Location;
	onDecide(decision: Decision): void;
	/** Closes the dialog and leaves the job waiting; "Resolve…" on its row opens it again. */
	onLater(): void;
	/**
	 * Hook for choosing another destination when the volume is full. Where it is given, "Choose
	 * another location…" is offered for a `notEnoughSpace` error; where it is not (no destination
	 * picker exists in this window), the button is left out.
	 */
	onChooseLocation?: () => void;
}

const LABEL: Record<Decision, MessageId> = {
	retry: 'ops.problem.retry',
	skip: 'ops.problem.skip',
	skipAll: 'ops.problem.skipAll',
	cancel: 'ops.problem.cancel',
	createParents: 'ops.problem.createParents',
};

/**
 * Says what went wrong in plain words with the item's place, and offers the decisions that fit.
 * Focus starts on the button that is safe and useful: "Recreate folders" when only a missing
 * folder stands in the way, otherwise "Retry". "Skip all like this" is never the default.
 * Escape closes the dialog and leaves the job waiting, because stopping a job part-way is not
 * something a stray key should do; "Cancel the operation" is its own button.
 */
export function OperationErrorDialog({
	job,
	error,
	item,
	onDecide,
	onLater,
	onChooseLocation,
}: OperationErrorDialogProps) {
	const { message, details } = problemText(error);
	const decisions = decisionsFor(error);
	const first: Decision = decisions[0] ?? 'retry';
	const missingParent = error.kind === 'originMissingParent';

	return (
		<Dialog
			open
			size="medium"
			title={t(missingParent ? 'ops.problem.title.parent' : 'ops.problem.title')}
			description={message}
			onClose={(reason) => {
				if (reason !== 'backdrop') onLater();
			}}
			initialFocus={`[data-decision="${first}"]`}
			footer={
				<DialogActions>
					<DialogButton variant="secondary" onClick={onLater}>
						{t('ops.problem.later')}
					</DialogButton>
					{onChooseLocation && error.kind === 'notEnoughSpace' && (
						<DialogButton variant="secondary" onClick={onChooseLocation}>
							{t('ops.problem.chooseLocation')}
						</DialogButton>
					)}
					{decisions.map((decision) => (
						<DialogButton
							key={decision}
							variant={decision === first ? 'primary' : 'secondary'}
							data-decision={decision}
							onClick={() => onDecide(decision)}
						>
							{t(LABEL[decision])}
						</DialogButton>
					))}
				</DialogActions>
			}
		>
			<div className={styles.body}>
				<p className={styles.fact} data-selectable="">
					{tf('ops.problem.item', { item: item.display })}
				</p>
				<p className={styles.fact} data-selectable="">
					{tf('ops.problem.job', { title: jobTitle(job) })}
				</p>
				{details.length > 0 && (
					<details className={styles.details}>
						<summary>{t('ops.problem.details')}</summary>
						{details.map((line) => (
							<p key={line} className={styles.detail} data-selectable="">
								{line}
							</p>
						))}
					</details>
				)}
				<p className={styles.hint}>{t('ops.problem.skipAll.hint')}</p>
			</div>
		</Dialog>
	);
}
