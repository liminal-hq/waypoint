// The ghost that follows the pointer in a file drag: a stack of up to three icons, a count, the action's badge and the pill
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { CSSProperties, ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { useStore } from 'zustand';
import { FileIcon } from '../browse/FileIcon';
import { t } from '../i18n/messages';
import { PlusIcon } from '../icons/AppIcons';
import { LinkIcon, MoveToIcon, TrashIcon } from '../icons/MenuIcons';
import type { DragSession } from './dragSession';
import type { FileDragSource, FileDropTarget, PillKind } from './fileDragModel';
import styles from './DragStack.module.css';

/** The icon cards behind the pressed row's, so a crowd looks like a crowd. */
const MAX_CARDS = 3;
const COUNT_CAP = 999;

const BADGES: Partial<Record<PillKind, ReactNode>> = {
	copy: <PlusIcon />,
	pending: <PlusIcon />,
	move: <MoveToIcon />,
	link: <LinkIcon />,
	trash: <TrashIcon />,
	open: <PlusIcon />,
	shelf: <PlusIcon />,
	ask: (
		<svg viewBox="0 0 16 16" aria-hidden="true" focusable="false" fill="currentColor">
			<circle cx="3.5" cy="8" r="1.4" />
			<circle cx="8" cy="8" r="1.4" />
			<circle cx="12.5" cy="8" r="1.4" />
		</svg>
	),
	blocked: (
		<svg
			viewBox="0 0 16 16"
			aria-hidden="true"
			focusable="false"
			fill="none"
			stroke="currentColor"
			strokeWidth="1.6"
			strokeLinecap="round"
		>
			<circle cx="8" cy="8" r="5.5" />
			<path d="M4.2 11.8l7.6-7.6" />
		</svg>
	),
};

export interface DragStackProps {
	session: DragSession<FileDragSource, FileDropTarget>;
}

/**
 * Drawn, not announced: the live region says the same words, so this is `aria-hidden`. It follows
 * the pointer through `--wp-drag-x` and `--wp-drag-y`, which the drag session writes, so moving
 * it renders nothing; React draws only when the pill or the phase changes.
 */
export function DragStack({ session }: DragStackProps) {
	const { phase, pill, source } = useStore(session.store);
	if (phase !== 'dragging' || !source || !pill) return null;
	const cards = source.groups.slice(0, MAX_CARDS);
	const count = source.count > COUNT_CAP ? `${COUNT_CAP}+` : String(source.count);
	return createPortal(
		<div className={styles.drag} aria-hidden="true" data-drag-stack="" data-kind={pill.kind}>
			<div className={styles.stack}>
				{cards
					.map((group, depth) => (
						<span
							key={depth}
							className={styles.card}
							style={{ '--wp-stack-depth': depth } as CSSProperties}
						>
							<FileIcon group={group} />
						</span>
					))
					.reverse()}
				{source.count > 1 && <span className={styles.count}>{count}</span>}
				{BADGES[pill.kind as PillKind] && (
					<span className={styles.badge} data-kind={pill.kind}>
						{BADGES[pill.kind as PillKind]}
					</span>
				)}
			</div>
			<div className={styles.pill} data-kind={pill.kind} data-drag-pill="">
				<span className={styles.text}>{pill.text}</span>
				<span className={styles.hint}>{t('drag.pill.esc')}</span>
			</div>
		</div>,
		document.body,
	);
}
