// App mark and menu button, openable from the keyboard with Alt or F10
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Fragment, useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import { ContextMenu, type ContextMenuProps } from '../ContextMenu/ContextMenu';
import type { MenuItem, MenuPosition, SelectableMenuItem } from '../ContextMenu/types';
import '../tokens.css';
import styles from './AppMenuButton.module.css';

export interface AppMenuButtonProps {
	/** App name shown beside the mark; also the button's accessible name. */
	label: string;
	/** App mark rendered before the name. */
	mark?: ReactNode;
	items: MenuItem[];
	onSelect: (item: SelectableMenuItem) => void;
	/** Toggles on F10 and on a lone Alt press. Defaults to true. */
	acceleratorKeys?: boolean;
	/**
	 * Alt plus a letter opens the menu with that top-level submenu open: the keys are lower-case
	 * letters and the values the ids of submenu items of `items` (`{ f: 'menu:file' }`). Only
	 * active with `acceleratorKeys`.
	 */
	mnemonics?: Readonly<Record<string, string>>;
	/**
	 * Draws the mark and name as plain text that opens nothing, with no accelerators: for when
	 * something else (a menu bar) carries the menus. Defaults to true.
	 */
	interactive?: boolean;
	/**
	 * Draws the menu this button opens, in place of the chrome's own: how a host shows it as the
	 * system's menu. It is given the props the chrome's menu takes, including `openedWithKeyboard`
	 * and, for an Alt mnemonic, the `initialSubmenuId` to open on.
	 */
	renderMenu?: (props: ContextMenuProps) => ReactNode;
}

export function AppMenuButton({
	label,
	mark,
	items,
	onSelect,
	acceleratorKeys = true,
	mnemonics,
	interactive = true,
	renderMenu: renderHostedMenu,
}: AppMenuButtonProps) {
	const buttonRef = useRef<HTMLButtonElement>(null);
	// A press on the button while its menu is up first dismisses the menu (the press is outside it),
	// so the click that follows must close rather than open again.
	const openOnPress = useRef(false);
	const [open, setOpen] = useState<{
		position: MenuPosition;
		viaKeyboard: boolean;
		submenu?: string;
		/** Counts the openings, so a second mnemonic while the menu is up opens a fresh menu. */
		serial: number;
	} | null>(null);

	// The accelerator listeners read this rather than the state, so they need not re-subscribe on
	// every opening and always see whether the menu is up now.
	const isOpen = useRef(false);
	isOpen.current = open !== null;

	const openMenu = useCallback((viaKeyboard: boolean, submenu?: string) => {
		const box = buttonRef.current?.getBoundingClientRect();
		setOpen((previous) => ({
			position: { x: box?.left ?? 0, y: box?.bottom ?? 0 },
			viaKeyboard,
			...(submenu ? { submenu } : {}),
			serial: (previous?.serial ?? 0) + 1,
		}));
	}, []);

	useEffect(() => {
		if (!acceleratorKeys || !interactive) return;
		let altAlone = false;
		const onKeyDown = (event: KeyboardEvent) => {
			if (event.key === 'F10' && !event.shiftKey) {
				event.preventDefault();
				altAlone = false;
				// F10 toggles: the menu's own unmount returns focus to where it was.
				if (isOpen.current) setOpen(null);
				else openMenu(true);
				return;
			}
			const target =
				event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey
					? mnemonics?.[event.key.toLowerCase()]
					: undefined;
			if (target) {
				event.preventDefault();
				altAlone = false;
				openMenu(true, target);
				return;
			}
			altAlone = event.key === 'Alt' && !event.repeat && !event.ctrlKey && !event.metaKey;
		};
		const onKeyUp = (event: KeyboardEvent) => {
			if (event.key === 'Alt' && altAlone) {
				altAlone = false;
				if (isOpen.current) setOpen(null);
				else openMenu(true);
			}
		};
		// Captured, because the open menu stops its keys from bubbling to the window: a second Alt
		// or F10 pressed with focus inside the menu must still reach this.
		window.addEventListener('keydown', onKeyDown, true);
		window.addEventListener('keyup', onKeyUp, true);
		return () => {
			window.removeEventListener('keydown', onKeyDown, true);
			window.removeEventListener('keyup', onKeyUp, true);
		};
	}, [acceleratorKeys, interactive, mnemonics, openMenu]);

	const renderMenu = (shown: NonNullable<typeof open>) => {
		const props: ContextMenuProps = {
			items,
			position: shown.position,
			ariaLabel: label,
			returnFocusTo: buttonRef.current,
			openedWithKeyboard: shown.viaKeyboard,
			...(shown.submenu ? { initialSubmenuId: shown.submenu } : {}),
			onSelect,
			onClose: () => setOpen(null),
		};
		return renderHostedMenu ? (
			<Fragment key={shown.serial}>{renderHostedMenu(props)}</Fragment>
		) : (
			<ContextMenu key={shown.serial} {...props} />
		);
	};

	if (!interactive) {
		return (
			<span className={`${styles.button} ${styles.plain}`}>
				{mark ? (
					<span className={styles.mark} aria-hidden="true">
						{mark}
					</span>
				) : null}
				<span>{label}</span>
			</span>
		);
	}

	return (
		<>
			<button
				ref={buttonRef}
				type="button"
				className={styles.button}
				aria-haspopup="menu"
				aria-expanded={open !== null}
				data-window-menu-exclude=""
				onPointerDown={() => {
					openOnPress.current = open !== null;
				}}
				onClick={() => {
					const wasOpen = openOnPress.current || open !== null;
					openOnPress.current = false;
					if (wasOpen) setOpen(null);
					else openMenu(false);
				}}
			>
				{mark ? (
					<span className={styles.mark} aria-hidden="true">
						{mark}
					</span>
				) : null}
				<span>{label}</span>
			</button>
			{open ? renderMenu(open) : null}
		</>
	);
}
