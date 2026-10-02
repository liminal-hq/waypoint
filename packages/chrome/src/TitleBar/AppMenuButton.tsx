// App mark and menu button, openable from the keyboard with Alt or F10
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import { ContextMenu } from '../ContextMenu/ContextMenu';
import type { MenuItem, MenuPosition, SelectableMenuItem } from '../ContextMenu/types';
import '../tokens.css';
import styles from './AppMenuButton.module.css';

export interface AppMenuButtonProps {
	/** App name shown beside the mark; also the button's accessible name. */
	label: string;
	/** App mark rendered before the name. */
	mark?: ReactNode;
	/** Whether the name shows beside the mark; hidden, it stays the button's accessible name. Defaults to true. */
	showLabel?: boolean;
	items: MenuItem[];
	onSelect: (item: SelectableMenuItem) => void;
	/** Opens on F10 and on a lone Alt press. Defaults to true. */
	acceleratorKeys?: boolean;
	/**
	 * Alt plus a letter opens the menu with that top-level submenu open: the keys are lower-case
	 * letters and the values the ids of submenu items of `items` (`{ f: 'menu:file' }`). Only
	 * active with `acceleratorKeys`.
	 */
	mnemonics?: Readonly<Record<string, string>>;
}

export function AppMenuButton({
	label,
	mark,
	showLabel = true,
	items,
	onSelect,
	acceleratorKeys = true,
	mnemonics,
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
		if (!acceleratorKeys) return;
		let altAlone = false;
		const onKeyDown = (event: KeyboardEvent) => {
			if (event.key === 'F10' && !event.shiftKey) {
				event.preventDefault();
				openMenu(true);
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
				openMenu(true);
			}
		};
		window.addEventListener('keydown', onKeyDown);
		window.addEventListener('keyup', onKeyUp);
		return () => {
			window.removeEventListener('keydown', onKeyDown);
			window.removeEventListener('keyup', onKeyUp);
		};
	}, [acceleratorKeys, mnemonics, openMenu]);

	return (
		<>
			<button
				ref={buttonRef}
				type="button"
				className={styles.button}
				aria-label={showLabel ? undefined : label}
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
				{showLabel ? <span>{label}</span> : null}
			</button>
			{open ? (
				<ContextMenu
					key={open.serial}
					items={items}
					position={open.position}
					ariaLabel={label}
					returnFocusTo={buttonRef.current}
					openedWithKeyboard={open.viaKeyboard}
					{...(open.submenu ? { initialSubmenuId: open.submenu } : {})}
					onSelect={onSelect}
					onClose={() => setOpen(null)}
				/>
			) : null}
		</>
	);
}
