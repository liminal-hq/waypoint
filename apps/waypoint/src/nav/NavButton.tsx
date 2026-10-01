// A toolbar button that can show a history menu: on right-click, long press or the menu key
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ContextMenu } from '@liminal-hq/waypoint-chrome/ContextMenu';
import type { MenuItem, MenuPosition } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import {
	useEffect,
	useRef,
	useState,
	type KeyboardEvent,
	type MouseEvent,
	type PointerEvent as ReactPointerEvent,
	type ReactNode,
} from 'react';
import styles from './NavButton.module.css';

/** How long the pointer stays down before the history menu opens. */
export const LONG_PRESS_MS = 500;
const MAX_MENU_ITEMS = 25;

interface NavButtonProps {
	label: string;
	/** The tooltip; defaults to the label (use it to add the shortcut). */
	title?: string;
	/** Makes the button a toggle, pressed while true. */
	pressed?: boolean;
	icon: ReactNode;
	disabled: boolean;
	onPress: () => void;
	/** Folders this button can jump to, nearest first. Without any, there is no menu. */
	history?: readonly Location[];
	menuLabel?: string;
	/** Jumps to the `index`th folder of `history`. */
	onPick?: (index: number) => void;
}

interface OpenMenu {
	position: MenuPosition;
	keyboard: boolean;
}

export function NavButton({
	label,
	title,
	pressed,
	icon,
	disabled,
	onPress,
	history = [],
	menuLabel,
	onPick,
}: NavButtonProps) {
	const button = useRef<HTMLButtonElement | null>(null);
	const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
	const longPressed = useRef(false);
	const [menu, setMenu] = useState<OpenMenu | null>(null);

	useEffect(() => () => clearTimer(), []);

	const clearTimer = () => {
		if (timer.current) clearTimeout(timer.current);
		timer.current = null;
	};

	const anchor = (): MenuPosition => {
		const rect = button.current?.getBoundingClientRect();
		return { x: rect?.left ?? 0, y: rect?.bottom ?? 0 };
	};

	const open = (position: MenuPosition, keyboard: boolean) => {
		if (history.length > 0) setMenu({ position, keyboard });
	};

	const onPointerDown = (event: ReactPointerEvent) => {
		if (event.button !== 0 || history.length === 0) return;
		longPressed.current = false;
		clearTimer();
		timer.current = setTimeout(() => {
			longPressed.current = true;
			open(anchor(), false);
		}, LONG_PRESS_MS);
	};

	const onClick = () => {
		// The click that ends a long press only closes the gesture; it must not also navigate.
		if (longPressed.current) {
			longPressed.current = false;
			return;
		}
		onPress();
	};

	const onContextMenu = (event: MouseEvent) => {
		event.preventDefault();
		open({ x: event.clientX, y: event.clientY }, false);
	};

	const onKeyDown = (event: KeyboardEvent) => {
		if (event.key === 'ContextMenu' || (event.key === 'F10' && event.shiftKey)) {
			event.preventDefault();
			open(anchor(), true);
		}
	};

	const items: MenuItem[] = [
		...(menuLabel ? [{ type: 'section', label: menuLabel } as const] : []),
		...history.slice(0, MAX_MENU_ITEMS).map((location, index): MenuItem => ({
			type: 'action',
			id: String(index),
			label: location.display,
		})),
	];

	return (
		<>
			<button
				ref={button}
				type="button"
				className={styles.button}
				aria-label={label}
				title={title ?? label}
				aria-pressed={pressed}
				disabled={disabled}
				aria-haspopup={history.length > 0 ? 'menu' : undefined}
				onClick={onClick}
				onPointerDown={onPointerDown}
				onPointerUp={clearTimer}
				onPointerLeave={clearTimer}
				onPointerCancel={clearTimer}
				onContextMenu={onContextMenu}
				onKeyDown={onKeyDown}
			>
				{icon}
			</button>
			{menu && (
				<ContextMenu
					items={items}
					position={menu.position}
					ariaLabel={menuLabel ?? label}
					openedWithKeyboard={menu.keyboard}
					returnFocusTo={button.current}
					onSelect={(item) => {
						setMenu(null);
						onPick?.(Number(item.id));
					}}
					onClose={() => setMenu(null)}
				/>
			)}
		</>
	);
}
