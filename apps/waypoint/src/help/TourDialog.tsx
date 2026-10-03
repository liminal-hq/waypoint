// The tour: five short steps on the ideas that set Waypoint apart, moved through with Back and Next
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Dialog } from '@liminal-hq/waypoint-chrome/Dialog/Dialog';
import { DialogActions, DialogButton } from '@liminal-hq/waypoint-chrome/Dialog/DialogActions';
import { useMemo, useState } from 'react';
import { t, tf } from '../i18n/messages';
import styles from './Help.module.css';
import { TOUR_LENGTH, tourSteps } from './helpModel';

interface TourDialogProps {
	onClose: () => void;
}

/**
 * One dialog whose heading and text change with the step. The step line is a status region, so a
 * screen reader hears the new step announced when Next or Back moves it, and focus stays on the
 * button that was pressed. Skip, Esc and Get Started all close it; nothing is remembered.
 */
export function TourDialog({ onClose }: TourDialogProps) {
	const steps = useMemo(tourSteps, []);
	const [index, setIndex] = useState(0);
	const step = steps[index]!;
	const last = index === TOUR_LENGTH - 1;
	return (
		<Dialog
			open
			onClose={onClose}
			title={step.title}
			description={step.text}
			footer={
				<DialogActions>
					<DialogButton onClick={onClose}>{t('tour.skip')}</DialogButton>
					{index > 0 && (
						<DialogButton key="back" onClick={() => setIndex(index - 1)}>
							{t('tour.back')}
						</DialogButton>
					)}
					<DialogButton
						key="next"
						variant="primary"
						onClick={last ? onClose : () => setIndex(index + 1)}
					>
						{t(last ? 'tour.done' : 'tour.next')}
					</DialogButton>
				</DialogActions>
			}
		>
			<p className={styles.step} role="status">
				{tf('tour.progress', { step: index + 1, total: TOUR_LENGTH })}
			</p>
		</Dialog>
	);
}
