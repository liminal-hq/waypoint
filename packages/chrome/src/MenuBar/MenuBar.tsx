// A permanent menu bar: a row of menus that open like the application menu's submenus, with the menu bar keyboard model
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import {
	useCallback,
	useEffect,
	useLayoutEffect,
	useMemo,
	useRef,
	useState,
	type KeyboardEvent,
} from 'react';
import { ContextMenu } from '../ContextMenu/ContextMenu';
import type { MenuItem, MenuPosition, SelectableMenuItem } from '../ContextMenu/types';
import '../tokens.css';
import { fitCount } from './fitCount';
import styles from './MenuBar.module.css';

export interface MenuBarProps {
	/** The menus: the submenu items of `items` are the bar's buttons, and anything else is left out. */
	items: MenuItem[];
	onSelect: (item: SelectableMenuItem) => void;
	/** The bar's accessible name. */
	label: string;
	/** The overflow button's name. */
	moreLabel: string;
	/**
	 * Alt plus a letter opens that menu: lower-case letters to the ids of submenu items of
	 * `items`. A lone Alt press or F10 moves focus into the bar, and again returns it.
	 */
	mnemonics?: Readonly<Record<string, string>>;
}

const MORE_ID = '\u0000more';
/** The size of the More button before it has been drawn, so the first measure leaves room for it. */
const MORE_WIDTH_GUESS = 36;

interface OpenMenu {
	/** A menu's id, or the More button's. */
	id: string;
	viaKeyboard: boolean;
	/** Counts the openings, so a menu opened again is a fresh one. */
	serial: number;
	/** The button the menu hangs from (More, for a menu that has collapsed into it). */
	anchorId: string;
}

const px = (value: string) => Number.parseFloat(value) || 0;

/**
 * A `menubar` of `menuitem` buttons. Left and Right move along it (Home and End jump), Down, Enter
 * or Space opens a menu, and Escape closes it with focus back on its button. While a menu is
 * open Left and Right (on a row that opens nothing) switch to the neighbouring menu, and hovering
 * another button opens that one. A lone Alt press or F10 focuses the first menu and, pressed
 * again or followed by Escape, returns focus to the element it left; with a menu open it closes
 * the menu. Menus that do not fit collapse, last first, into a More menu.
 */
export function MenuBar({ items, onSelect, label, moreLabel, mnemonics }: MenuBarProps) {
	const menus = useMemo(
		() => items.flatMap((item) => (item.type === 'submenu' && !item.disabled ? [item] : [])),
		[items],
	);
	const barRef = useRef<HTMLDivElement>(null);
	const cellRefs = useRef<Array<HTMLDivElement | null>>([]);
	const buttonRefs = useRef<Array<HTMLButtonElement | null>>([]);
	const moreRef = useRef<HTMLDivElement>(null);
	const [shown, setShown] = useState(Number.POSITIVE_INFINITY);
	const [focusId, setFocusId] = useState<string | null>(null);
	const [open, setOpen] = useState<OpenMenu | null>(null);
	/** Where focus was before the keyboard took it into the bar. */
	const previous = useRef<HTMLElement | null>(null);

	const visible = Math.min(shown, menus.length);
	const overflowed = menus.slice(visible);
	const stopIds = [
		...menus.slice(0, visible).map((menu) => menu.id),
		...(overflowed.length > 0 ? [MORE_ID] : []),
	];
	const stopId = focusId !== null && stopIds.includes(focusId) ? focusId : (stopIds[0] ?? null);

	// Measures the buttons (hidden ones included, which stay in the document) and decides how many fit.
	const signature = menus.map((menu) => `${menu.id}:${menu.label}`).join('|');
	const measure = useCallback(() => {
		const bar = barRef.current;
		if (!bar) return;
		const widths = menus.map((_, index) => cellRefs.current[index]?.offsetWidth ?? 0);
		// Not laid out (no layout engine): nothing to decide, show everything.
		if (widths.every((width) => width === 0)) return;
		const style = getComputedStyle(bar);
		const available = bar.clientWidth - px(style.paddingLeft) - px(style.paddingRight);
		const more = moreRef.current?.offsetWidth || MORE_WIDTH_GUESS;
		const count = fitCount(widths, available, px(style.columnGap), more);
		setShown((last) => (Math.min(last, menus.length) === count ? last : count));
	}, [menus]);
	useLayoutEffect(() => {
		measure();
		const bar = barRef.current;
		if (!bar || typeof ResizeObserver === 'undefined') return;
		const observer = new ResizeObserver(() => measure());
		observer.observe(bar);
		return () => observer.disconnect();
		// `signature` stands for "the menus or their labels changed".
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [measure, signature]);

	const buttonFor = (id: string): HTMLButtonElement | null => {
		if (id === MORE_ID) return buttonRefs.current[menus.length] ?? null;
		const index = menus.findIndex((menu) => menu.id === id);
		return index >= 0 && index < visible ? (buttonRefs.current[index] ?? null) : null;
	};

	const openMenu = useCallback(
		(id: string, viaKeyboard: boolean) => {
			const index = menus.findIndex((menu) => menu.id === id);
			const anchorId = id === MORE_ID || index < 0 || index >= visible ? MORE_ID : id;
			setFocusId(anchorId);
			setOpen((last) => ({ id, viaKeyboard, anchorId, serial: (last?.serial ?? 0) + 1 }));
		},
		[menus, visible],
	);

	const focusStop = (id: string) => {
		setFocusId(id);
		buttonFor(id)?.focus();
	};

	const leaveBar = useCallback(() => {
		const target = previous.current;
		previous.current = null;
		if (target?.isConnected) target.focus();
		else if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
	}, []);

	const enterBar = useCallback(() => {
		const active = document.activeElement;
		if (!barRef.current?.contains(active)) {
			previous.current = active instanceof HTMLElement && active !== document.body ? active : null;
		}
	}, []);

	const openRef = useRef(open);
	openRef.current = open;
	const stopsRef = useRef({ stopIds, buttonFor });
	stopsRef.current = { stopIds, buttonFor };

	// The accelerators are captured: an open menu stops its keys from bubbling to the window.
	useEffect(() => {
		let altAlone = false;
		const toggle = () => {
			if (openRef.current) {
				// Close the menu; its unmount puts focus back on its button.
				setOpen(null);
				return;
			}
			if (barRef.current?.contains(document.activeElement)) {
				leaveBar();
				return;
			}
			const first = stopsRef.current.stopIds[0];
			if (first === undefined) return;
			enterBar();
			setFocusId(first);
			stopsRef.current.buttonFor(first)?.focus();
		};
		const onKeyDown = (event: globalThis.KeyboardEvent) => {
			if (event.key === 'F10' && !event.shiftKey) {
				event.preventDefault();
				altAlone = false;
				toggle();
				return;
			}
			const target =
				event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey
					? mnemonics?.[event.key.toLowerCase()]
					: undefined;
			if (target && menus.some((menu) => menu.id === target)) {
				event.preventDefault();
				altAlone = false;
				if (!openRef.current) enterBar();
				openMenu(target, true);
				return;
			}
			altAlone = event.key === 'Alt' && !event.repeat && !event.ctrlKey && !event.metaKey;
		};
		const onKeyUp = (event: globalThis.KeyboardEvent) => {
			if (event.key === 'Alt' && altAlone) {
				altAlone = false;
				toggle();
			}
		};
		window.addEventListener('keydown', onKeyDown, true);
		window.addEventListener('keyup', onKeyUp, true);
		return () => {
			window.removeEventListener('keydown', onKeyDown, true);
			window.removeEventListener('keyup', onKeyUp, true);
		};
	}, [enterBar, leaveBar, menus, mnemonics, openMenu]);

	const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
		const current = stopIds.find((id) => buttonFor(id) === document.activeElement);
		if (current === undefined || open) return;
		const index = stopIds.indexOf(current);
		const step = (by: number) =>
			focusStop(stopIds[(index + by + stopIds.length) % stopIds.length]!);
		switch (event.key) {
			case 'ArrowRight':
				event.preventDefault();
				step(1);
				return;
			case 'ArrowLeft':
				event.preventDefault();
				step(-1);
				return;
			case 'Home':
				event.preventDefault();
				focusStop(stopIds[0]!);
				return;
			case 'End':
				event.preventDefault();
				focusStop(stopIds[stopIds.length - 1]!);
				return;
			case 'ArrowDown':
			case 'Enter':
			case ' ':
				event.preventDefault();
				openMenu(current, true);
				return;
			case 'Escape':
				event.preventDefault();
				leaveBar();
		}
	};

	/** Switches the open menu to its neighbour along the bar, wrapping at the ends. */
	const sideways = (direction: -1 | 1) => {
		if (!open) return;
		const at = Math.max(stopIds.indexOf(open.anchorId), 0);
		const next = stopIds[(at + direction + stopIds.length) % stopIds.length]!;
		openMenu(next, true);
	};

	const menuItems = (id: string): MenuItem[] => {
		if (id === MORE_ID) return overflowed;
		const menu = menus.find((candidate) => candidate.id === id);
		return menu?.items ?? [];
	};

	const anchorPosition = (anchorId: string): MenuPosition => {
		const box = buttonFor(anchorId)?.getBoundingClientRect();
		return { x: box?.left ?? 0, y: box?.bottom ?? 0 };
	};

	const nameOf = (id: string) =>
		id === MORE_ID ? moreLabel : (menus.find((menu) => menu.id === id)?.label ?? label);

	const buttonProps = (id: string, hovering: boolean) => ({
		role: 'menuitem' as const,
		'aria-haspopup': 'menu' as const,
		'aria-expanded': open?.anchorId === id,
		tabIndex: stopId === id ? 0 : -1,
		onFocus: () => setFocusId(id),
		// Only one press at a time can start a menu: a press on the button whose menu is up first
		// dismisses that menu, and the menu swallows the click that follows.
		onClick: () => openMenu(id, false),
		onMouseEnter: () => {
			if (hovering && open && open.anchorId !== id) openMenu(id, false);
		},
	});

	const aria = (id: string) => {
		const letter = Object.entries(mnemonics ?? {}).find(([, target]) => target === id)?.[0];
		return letter ? { 'aria-keyshortcuts': `Alt+${letter.toUpperCase()}` } : {};
	};

	return (
		<>
			<div
				ref={barRef}
				className={styles.bar}
				role="menubar"
				aria-label={label}
				aria-orientation="horizontal"
				onKeyDown={onKeyDown}
			>
				{menus.map((menu, index) => {
					const onBar = index < visible;
					return (
						<div
							key={menu.id}
							ref={(element) => {
								cellRefs.current[index] = element;
							}}
							className={styles.cell}
							data-overflow={onBar ? undefined : ''}
						>
							<button
								ref={(element) => {
									buttonRefs.current[index] = element;
								}}
								type="button"
								className={styles.button}
								data-menu={menu.id}
								{...buttonProps(menu.id, onBar)}
								{...(onBar ? aria(menu.id) : { tabIndex: -1 })}
							>
								{menu.label}
							</button>
						</div>
					);
				})}
				{overflowed.length > 0 && (
					<div ref={moreRef} className={styles.cell}>
						<button
							ref={(element) => {
								buttonRefs.current[menus.length] = element;
							}}
							type="button"
							className={styles.button}
							data-menu="more"
							{...buttonProps(MORE_ID, true)}
						>
							{moreLabel}
						</button>
					</div>
				)}
			</div>
			{open ? (
				<ContextMenu
					key={open.serial}
					items={menuItems(open.id)}
					position={anchorPosition(open.anchorId)}
					ariaLabel={nameOf(open.id)}
					returnFocusTo={buttonFor(open.anchorId)}
					openedWithKeyboard={open.viaKeyboard}
					onSideways={sideways}
					onSelect={onSelect}
					onClose={() => setOpen(null)}
				/>
			) : null}
		</>
	);
}
