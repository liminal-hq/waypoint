// Controlled modal dialog over the native `<dialog>` element
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import {
	useCallback,
	useEffect,
	useId,
	useLayoutEffect,
	useMemo,
	useRef,
	useState,
	type KeyboardEvent as ReactKeyboardEvent,
	type MouseEvent as ReactMouseEvent,
	type ReactNode,
	type RefObject,
} from 'react';
import '../tokens.css';
import styles from './Dialog.module.css';
import { DialogContext, type DialogCloseReason } from './dialogContext';
import { defaultFocusTarget, trapTab } from './focus';
import { lockScroll, unlockScroll } from './scrollLock';

export type DialogSize = 'small' | 'medium' | 'large';

export interface DialogProps {
	open: boolean;
	/** Asked for when the dialog wants to close. The app owns `open`, so nothing closes until it says so. */
	onClose: (reason: DialogCloseReason) => void;
	/** The dialog's accessible name, also shown as its heading. */
	title: ReactNode;
	/** Supporting text, wired up as the accessible description. */
	description?: ReactNode;
	size?: DialogSize;
	children?: ReactNode;
	/** Buttons row; `DialogActions` is the usual content. */
	footer?: ReactNode;
	/** The element or CSS selector (inside the dialog) to focus on open. Defaults to the least destructive control. */
	initialFocus?: RefObject<HTMLElement | null> | string;
	/** When false, Esc and a backdrop click do nothing, for questions that must be answered. */
	dismissible?: boolean;
	/** Element that receives focus on close. Defaults to the element focused when the dialog opened. */
	returnFocusTo?: HTMLElement | null;
}

// Open dialogs, bottom first. Only the top one answers Esc and lifts above the others.
const stack: HTMLDialogElement[] = [];

/** True when `dialog` has another open dialog inside it, which is then the one in front. */
function hasOpenChild(dialog: HTMLDialogElement): boolean {
	return dialog.querySelector('dialog[open]') !== null;
}

/** The dialog in front: the latest one opened that has no open dialog nested inside it. */
function isTopmost(dialog: HTMLDialogElement): boolean {
	const candidates = stack.filter((entry) => !hasOpenChild(entry));
	return candidates[candidates.length - 1] === dialog;
}

/**
 * Whether the host asks for reduced motion: its `data-motion` attribute on the root (the host's
 * setting and the OS preference combined) or, before the host sets one, the media query.
 */
function prefersReducedMotion(): boolean {
	const attribute = globalThis.document?.documentElement.dataset.motion;
	if (attribute) return attribute === 'reduce';
	try {
		return globalThis.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false;
	} catch {
		return false;
	}
}

export function Dialog(props: DialogProps) {
	// Mounted only while open, so every open starts from a clean slate and cleanup is the close.
	return props.open ? <OpenDialog {...props} /> : null;
}

function OpenDialog({
	onClose,
	title,
	description,
	size = 'medium',
	children,
	footer,
	initialFocus,
	dismissible = true,
	returnFocusTo,
}: DialogProps) {
	const dialogRef = useRef<HTMLDialogElement>(null);
	const surfaceRef = useRef<HTMLDivElement>(null);
	const titleId = useId();
	const descriptionId = useId();
	const [depth, setDepth] = useState(0);
	const [native, setNative] = useState(true);
	const reducedMotion = useMemo(prefersReducedMotion, []);

	// Latest props for the long-lived listeners below.
	const live = useRef({ onClose, dismissible, returnFocusTo, initialFocus });
	useLayoutEffect(() => {
		live.current = { onClose, dismissible, returnFocusTo, initialFocus };
	});

	const escapeKeyAt = useRef(0);
	const downOnBackdrop = useRef(false);

	const requestClose = useCallback((reason: DialogCloseReason) => {
		live.current.onClose(reason);
	}, []);
	const context = useMemo(() => ({ requestClose }), [requestClose]);

	useLayoutEffect(() => {
		const dialog = dialogRef.current;
		const surface = surfaceRef.current;
		if (!dialog || !surface) return;
		const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
		stack.push(dialog);
		setDepth(stack.length - 1);
		lockScroll();

		// The platform gives the modal behaviour (top layer, inert page, focus trap, Esc). A webview
		// without `showModal` still gets an open dialog, and the Tab trap below covers focus.
		if (typeof dialog.showModal === 'function') {
			try {
				if (!dialog.open) dialog.showModal();
			} catch {
				dialog.setAttribute('open', '');
				setNative(false);
			}
		} else {
			dialog.setAttribute('open', '');
			setNative(false);
		}

		// A dialog nested in this one has already taken focus (its effect runs first); leave it there.
		if (!hasOpenChild(dialog)) {
			const { initialFocus: wanted } = live.current;
			let target: HTMLElement | null = null;
			if (typeof wanted === 'string') target = dialog.querySelector<HTMLElement>(wanted);
			else if (wanted) target = wanted.current;
			target ??= defaultFocusTarget(dialog);
			(target ?? surface).focus();
		}

		const onKeyDown = (event: KeyboardEvent) => {
			if (event.key !== 'Escape' || event.defaultPrevented) return;
			if (!isTopmost(dialog)) return;
			// Handled here so the result is the same with or without native Esc handling.
			event.preventDefault();
			escapeKeyAt.current = Date.now();
			if (live.current.dismissible) live.current.onClose('escape');
		};
		document.addEventListener('keydown', onKeyDown);

		return () => {
			document.removeEventListener('keydown', onKeyDown);
			const at = stack.indexOf(dialog);
			if (at >= 0) stack.splice(at, 1);
			if (dialog.open && typeof dialog.close === 'function') dialog.close();
			unlockScroll();
			const back = live.current.returnFocusTo ?? opener;
			if (back?.isConnected) back.focus();
		};
	}, []);

	const onCancel = (event: Event) => {
		// Controlled: the platform never closes the dialog on its own.
		event.preventDefault();
		if (Date.now() - escapeKeyAt.current < 100) return; // the Esc keydown already answered
		if (live.current.dismissible) live.current.onClose('cancel');
	};
	// `onCancel` is attached natively because React does not forward the `cancel` event reliably.
	useEffect(() => {
		const dialog = dialogRef.current;
		if (!dialog) return;
		dialog.addEventListener('cancel', onCancel);
		return () => dialog.removeEventListener('cancel', onCancel);
	}, []);

	const onKeyDown = (event: ReactKeyboardEvent<HTMLDialogElement>) => {
		if (event.key !== 'Tab' || !dialogRef.current || !surfaceRef.current) return;
		// A stacked dialog sits inside this one in the tree; only the innermost handles Tab.
		event.stopPropagation();
		trapTab(event.nativeEvent, dialogRef.current, surfaceRef.current);
	};

	const onMouseDown = (event: ReactMouseEvent<HTMLDialogElement>) => {
		downOnBackdrop.current = event.target === event.currentTarget;
		// A press on the backdrop must not pull focus out of the dialog onto its surface.
		if (downOnBackdrop.current) event.preventDefault();
	};
	const onClick = (event: ReactMouseEvent<HTMLDialogElement>) => {
		// The backdrop belongs to the dialog element, so a click on it targets the dialog itself. Both
		// the press and the release must land there, so dragging a text selection out does not close.
		const onBackdrop = event.target === event.currentTarget && downOnBackdrop.current;
		downOnBackdrop.current = false;
		if (onBackdrop && live.current.dismissible) live.current.onClose('backdrop');
	};

	const classes = [
		styles.dialog,
		styles[size],
		native ? '' : styles.fallback,
		reducedMotion ? styles.reducedMotion : '',
	]
		.filter(Boolean)
		.join(' ');

	return (
		<DialogContext.Provider value={context}>
			<dialog
				ref={dialogRef}
				className={classes}
				aria-labelledby={titleId}
				aria-describedby={description ? descriptionId : undefined}
				data-size={size}
				style={native ? undefined : { zIndex: `calc(var(--wp-z-dialog) + ${depth})` }}
				onKeyDown={onKeyDown}
				onMouseDown={onMouseDown}
				onClick={onClick}
			>
				<div ref={surfaceRef} className={styles.surface} tabIndex={-1}>
					<header className={styles.header}>
						<h2 id={titleId} className={styles.title}>
							{title}
						</h2>
						{description ? (
							<p id={descriptionId} className={styles.description}>
								{description}
							</p>
						) : null}
					</header>
					{children ? (
						<div className={styles.body} data-dialog-body>
							{children}
						</div>
					) : null}
					{footer ? <footer className={styles.footer}>{footer}</footer> : null}
				</div>
			</dialog>
		</DialogContext.Provider>
	);
}
