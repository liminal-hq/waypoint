// App mark and menu button, openable from the keyboard with Alt or F10
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import { ContextMenu } from '../ContextMenu/ContextMenu';
import type { MenuItem, MenuPosition, SelectableMenuItem } from '../ContextMenu/types';
import styles from './AppMenuButton.module.css';

export interface AppMenuButtonProps {
	/** App name shown beside the mark; also the button's accessible name. */
	label: string;
	/** App mark rendered before the name. */
	mark?: ReactNode;
	items: MenuItem[];
	onSelect: (item: SelectableMenuItem) => void;
	/** Opens on F10 and on a lone Alt press. Defaults to true. */
	acceleratorKeys?: boolean;
}

export function AppMenuButton({
	label,
	mark,
	items,
	onSelect,
	acceleratorKeys = true,
}: AppMenuButtonProps) {
	const buttonRef = useRef<HTMLButtonElement>(null);
	const [open, setOpen] = useState<{ position: MenuPosition; viaKeyboard: boolean } | null>(null);

	const openMenu = useCallback((viaKeyboard: boolean) => {
		const box = buttonRef.current?.getBoundingClientRect();
		setOpen({ position: { x: box?.left ?? 0, y: box?.bottom ?? 0 }, viaKeyboard });
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
	}, [acceleratorKeys, openMenu]);

	return (
		<>
			<button
				ref={buttonRef}
				type="button"
				className={styles.button}
				aria-haspopup="menu"
				aria-expanded={open !== null}
				data-window-menu-exclude=""
				onClick={() => (open ? setOpen(null) : openMenu(false))}
			>
				{mark ? (
					<span className={styles.mark} aria-hidden="true">
						{mark}
					</span>
				) : null}
				<span>{label}</span>
			</button>
			{open ? (
				<ContextMenu
					items={items}
					position={open.position}
					ariaLabel={label}
					returnFocusTo={buttonRef.current}
					openedWithKeyboard={open.viaKeyboard}
					onSelect={onSelect}
					onClose={() => setOpen(null)}
				/>
			) : null}
		</>
	);
}
