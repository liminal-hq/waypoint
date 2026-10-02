// Asks a question in the chrome's confirm dialog and hands the answer back as a promise
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ConfirmDialog } from '@liminal-hq/waypoint-chrome/Dialog/ConfirmDialog';
import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import { t } from '../i18n/messages';
import styles from './ConfirmHost.module.css';
import type { ConfirmSpec } from './fileCommands';

interface Pending {
	spec: ConfirmSpec;
	resolve: (answer: boolean) => void;
}

/**
 * One confirmation at a time. `confirm` resolves `true` for the confirm button and `false` for
 * Cancel, Esc and a click outside, and a second question asked while one is open answers the first
 * `false`. Render `dialog` once, anywhere under the window. A destructive question starts with
 * focus on Cancel (the dialog's default) so Enter never confirms it by accident.
 */
export function useConfirm(): {
	confirm: (spec: ConfirmSpec) => Promise<boolean>;
	dialog: ReactNode;
} {
	const [pending, setPending] = useState<Pending | null>(null);
	const current = useRef<Pending | null>(null);
	current.current = pending;

	const confirm = useCallback(
		(spec: ConfirmSpec) =>
			new Promise<boolean>((resolve) => {
				current.current?.resolve(false);
				setPending({ spec, resolve });
			}),
		[],
	);
	// A window that goes away with a question open answers it, so no command waits forever.
	useEffect(() => () => current.current?.resolve(false), []);

	const answer = (value: boolean) => {
		pending?.resolve(value);
		setPending(null);
	};
	const spec = pending?.spec;

	const dialog = (
		<ConfirmDialog
			open={pending !== null}
			title={spec?.title ?? ''}
			message={spec ? <ConfirmBody spec={spec} /> : null}
			confirmLabel={spec?.confirmLabel ?? ''}
			cancelLabel={spec?.cancelLabel ?? t('files.cancel')}
			danger={spec?.danger ?? false}
			onConfirm={() => answer(true)}
			onCancel={() => answer(false)}
		/>
	);
	return { confirm, dialog };
}

function ConfirmBody({ spec }: { spec: ConfirmSpec }) {
	return (
		<span className={styles.body}>
			<span className={styles.message}>{spec.message}</span>
			{spec.items && spec.items.length > 0 && (
				<ul className={styles.items} aria-label={t('files.delete.list.label')}>
					{spec.items.map((name, index) => (
						<li key={`${index}-${name}`} className={styles.item}>
							{name}
						</li>
					))}
				</ul>
			)}
			{spec.note && <span className={styles.note}>{spec.note}</span>}
		</span>
	);
}
