// Controlled context menu with full keyboard model, submenus and viewport clamping
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import {
	useCallback,
	useEffect,
	useLayoutEffect,
	useRef,
	useState,
	type CSSProperties,
	type KeyboardEvent as ReactKeyboardEvent,
} from 'react';
import { createPortal } from 'react-dom';
import { useOptionalWindowFocus } from '../WindowChromeProvider/WindowChromeProvider';
import { CheckIcon, ChevronRightIcon } from '../icons/icons';
import '../tokens.css';
import styles from './ContextMenu.module.css';
import { findByPrefix, firstIndex, isNavigable, lastIndex, stepIndex } from './navigation';
import { clampToViewport, placeSubmenu, type Rect } from './placement';
import type { MenuItem, MenuPosition, SelectableMenuItem, SubmenuMenuItem } from './types';

/** Delay before a hovered submenu row opens (or a sibling hover closes it). */
export const SUBMENU_HOVER_DELAY_MS = 150;
const TYPE_AHEAD_RESET_MS = 600;

export interface ContextMenuProps {
	items: MenuItem[];
	/** Viewport coordinates of the pointer (or the anchor corner for keyboard-opened menus). */
	position: MenuPosition;
	onSelect: (item: SelectableMenuItem) => void;
	onClose: () => void;
	/** Accessible name for the menu. */
	ariaLabel?: string;
	/** Element that receives focus when the menu closes. Defaults to the element focused at open. */
	returnFocusTo?: HTMLElement | null;
	/** Focuses the first item on open instead of the menu surface. */
	openedWithKeyboard?: boolean;
}

type Placement = { kind: 'point'; position: MenuPosition } | { kind: 'anchor'; rect: Rect };
type FocusTarget = 'panel' | 'first' | 'none';

/**
 * The measured menu position, handed to the stylesheet as custom properties. The layout rules stay
 * in `ContextMenu.module.css`; only the two numbers the browser measures are set from script.
 */
function menuPlacement(placed: MenuPosition): CSSProperties {
	return { '--wp-menu-x': `${placed.x}px`, '--wp-menu-y': `${placed.y}px` } as CSSProperties;
}

export function ContextMenu({
	items,
	position,
	onSelect,
	onClose,
	ariaLabel,
	returnFocusTo,
	openedWithKeyboard = false,
}: ContextMenuProps) {
	const [trigger] = useState<Element | null>(() => returnFocusTo ?? document.activeElement);
	const rootRef = useRef<HTMLDivElement>(null);
	const onCloseRef = useRef(onClose);
	onCloseRef.current = onClose;

	// Restore focus to whatever opened the menu when it goes away.
	useEffect(() => {
		return () => {
			if (trigger instanceof HTMLElement && trigger.isConnected && trigger !== document.body) {
				trigger.focus();
			}
		};
	}, [trigger]);

	// With a provider, focus loss is a focused-to-unfocused transition; without one the DOM
	// `blur` event stands in. A menu opened while the window is already unfocused stays open,
	// and so does one open while the provider is still reading the initial focus state.
	const windowFocus = useOptionalWindowFocus();
	const hasProvider = windowFocus !== undefined;
	const focused = windowFocus?.focused;
	const settled = windowFocus?.settled ?? false;
	const previousFocused = useRef<boolean | undefined>(settled ? focused : undefined);
	useEffect(() => {
		if (previousFocused.current === true && focused === false) onCloseRef.current();
		previousFocused.current = settled ? focused : undefined;
	}, [focused, settled]);

	useEffect(() => {
		const dismiss = () => onCloseRef.current();
		const onPointerDown = (event: Event) => {
			const target = event.target;
			if (target instanceof Element && target.closest('[data-wp-menu-root]')) return;
			dismiss();
		};
		document.addEventListener('pointerdown', onPointerDown, true);
		if (!hasProvider) window.addEventListener('blur', dismiss);
		window.addEventListener('resize', dismiss);
		return () => {
			document.removeEventListener('pointerdown', onPointerDown, true);
			window.removeEventListener('blur', dismiss);
			window.removeEventListener('resize', dismiss);
		};
	}, [hasProvider]);

	const handleSelect = useCallback(
		(item: SelectableMenuItem) => {
			onSelect(item);
			onClose();
		},
		[onSelect, onClose],
	);

	return createPortal(
		<div ref={rootRef} data-wp-menu-root="">
			<MenuPanel
				items={items}
				placement={{ kind: 'point', position }}
				ariaLabel={ariaLabel}
				focusTarget={openedWithKeyboard ? 'first' : 'panel'}
				nested={false}
				onSelect={handleSelect}
				onRequestClose={onClose}
				onDismiss={onClose}
			/>
		</div>,
		document.body,
	);
}

interface MenuPanelProps {
	items: MenuItem[];
	placement: Placement;
	ariaLabel?: string;
	focusTarget: FocusTarget;
	nested: boolean;
	onSelect: (item: SelectableMenuItem) => void;
	/** Close just this panel (Escape, or ArrowLeft in a submenu). */
	onRequestClose: () => void;
	/** Close the whole menu tree. */
	onDismiss: () => void;
}

interface OpenSubmenu {
	index: number;
	rect: Rect;
	viaKeyboard: boolean;
}

function MenuPanel({
	items,
	placement,
	ariaLabel,
	focusTarget,
	nested,
	onSelect,
	onRequestClose,
	onDismiss,
}: MenuPanelProps) {
	const panelRef = useRef<HTMLDivElement>(null);
	const itemRefs = useRef<(HTMLDivElement | null)[]>([]);
	const hoverTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
	const typeAhead = useRef<{ buffer: string; timer?: ReturnType<typeof setTimeout> }>({
		buffer: '',
	});
	const [openSub, setOpenSub] = useState<OpenSubmenu | null>(null);
	const [placed, setPlaced] = useState<MenuPosition>(
		placement.kind === 'point'
			? placement.position
			: { x: placement.rect.right, y: placement.rect.top },
	);

	// Measure before paint and keep the panel inside the viewport.
	useLayoutEffect(() => {
		const panel = panelRef.current;
		if (!panel) return;
		const box = panel.getBoundingClientRect();
		const size = { width: box.width, height: box.height };
		const viewport = { width: window.innerWidth, height: window.innerHeight };
		setPlaced(
			placement.kind === 'point'
				? clampToViewport(placement.position, size, viewport)
				: placeSubmenu(placement.rect, size, viewport),
		);
	}, [placement]);

	useEffect(() => {
		if (focusTarget === 'panel') panelRef.current?.focus({ preventScroll: true });
		if (focusTarget === 'first') {
			const first = firstIndex(items);
			if (first >= 0) focusItem(first);
			else panelRef.current?.focus({ preventScroll: true });
		}
		// Only on mount: later item changes must not steal focus.
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, []);

	useEffect(() => {
		const typing = typeAhead.current;
		return () => {
			clearTimeout(hoverTimer.current);
			clearTimeout(typing.timer);
		};
	}, []);

	// A menu taller than the viewport scrolls inside itself, so the focused item is scrolled into
	// view by hand: `preventScroll` keeps the browser from also scrolling the page behind it.
	const focusItem = (index: number) => {
		const item = index >= 0 ? itemRefs.current[index] : null;
		if (!item) return;
		item.focus({ preventScroll: true });
		item.scrollIntoView?.({ block: 'nearest' });
	};

	const currentIndex = () => {
		const active = document.activeElement;
		return itemRefs.current.findIndex((el) => el !== null && el === active);
	};

	const openSubmenu = (index: number, viaKeyboard: boolean) => {
		const el = itemRefs.current[index];
		if (!el) return;
		clearTimeout(hoverTimer.current);
		const box = el.getBoundingClientRect();
		setOpenSub({
			index,
			viaKeyboard,
			rect: { left: box.left, top: box.top, right: box.right, bottom: box.bottom },
		});
	};

	const closeSubmenu = (refocusParent: boolean) => {
		const index = openSub?.index ?? -1;
		setOpenSub(null);
		if (refocusParent) focusItem(index);
	};

	const activate = (index: number, viaKeyboard: boolean) => {
		const item = items[index];
		if (!item || !isNavigable(item)) return;
		if (item.type === 'submenu') openSubmenu(index, viaKeyboard);
		else if (item.type === 'action' || item.type === 'checkbox') onSelect(item);
	};

	const handleTypeAhead = (key: string) => {
		const state = typeAhead.current;
		clearTimeout(state.timer);
		state.buffer += key;
		state.timer = setTimeout(() => {
			state.buffer = '';
		}, TYPE_AHEAD_RESET_MS);
		let match = findByPrefix(items, state.buffer, currentIndex());
		if (match < 0 && state.buffer.length > 1) {
			// A repeated letter ("aa") cycles rather than extending the prefix.
			state.buffer = key;
			match = findByPrefix(items, key, currentIndex());
		}
		focusItem(match);
	};

	const onKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
		// Nested panels sit inside their parent in the DOM; each panel handles its own keys.
		event.stopPropagation();
		const current = currentIndex();
		switch (event.key) {
			case 'ArrowDown':
				event.preventDefault();
				focusItem(stepIndex(items, current, 1));
				return;
			case 'ArrowUp':
				event.preventDefault();
				focusItem(stepIndex(items, current, -1));
				return;
			case 'Home':
				event.preventDefault();
				focusItem(firstIndex(items));
				return;
			case 'End':
				event.preventDefault();
				focusItem(lastIndex(items));
				return;
			case 'Enter':
			case ' ':
				event.preventDefault();
				if (current >= 0) activate(current, true);
				return;
			case 'ArrowRight': {
				const item = items[current];
				if (item?.type === 'submenu' && isNavigable(item)) {
					event.preventDefault();
					openSubmenu(current, true);
				}
				return;
			}
			case 'ArrowLeft':
				if (nested) {
					event.preventDefault();
					onRequestClose();
				}
				return;
			case 'Escape':
				event.preventDefault();
				onRequestClose();
				return;
			case 'Tab':
				event.preventDefault();
				onDismiss();
				return;
			default:
				if (event.key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey) {
					event.preventDefault();
					handleTypeAhead(event.key);
				}
		}
	};

	const onItemEnter = (index: number, item: MenuItem) => {
		if (!isNavigable(item)) return;
		clearTimeout(hoverTimer.current);
		focusItem(index);
		if (item.type === 'submenu') {
			if (openSub?.index === index) return;
			hoverTimer.current = setTimeout(() => openSubmenu(index, false), SUBMENU_HOVER_DELAY_MS);
		} else if (openSub) {
			hoverTimer.current = setTimeout(() => setOpenSub(null), SUBMENU_HOVER_DELAY_MS);
		}
	};

	const openItem = openSub ? items[openSub.index] : undefined;
	const submenuItems = openItem?.type === 'submenu' ? (openItem as SubmenuMenuItem).items : null;

	return (
		<div
			ref={panelRef}
			className={styles.menu}
			role="menu"
			aria-label={ariaLabel}
			tabIndex={-1}
			style={menuPlacement(placed)}
			onKeyDown={onKeyDown}
			onContextMenu={(event) => event.preventDefault()}
		>
			{items.map((item, index) => {
				if (item.type === 'separator') {
					return (
						<div key={item.id ?? `sep-${index}`} className={styles.separator} role="separator" />
					);
				}
				if (item.type === 'section') {
					return (
						<div key={item.id ?? `sec-${index}`} className={styles.section} role="presentation">
							{item.label}
						</div>
					);
				}
				const isSubmenu = item.type === 'submenu';
				const isChecked = item.type === 'checkbox' && item.checked;
				const danger = item.type === 'action' && item.danger;
				const role = item.type === 'checkbox' ? 'menuitemcheckbox' : 'menuitem';
				const classes = [
					styles.item,
					danger ? styles.danger : '',
					item.disabled ? styles.disabled : '',
				]
					.filter(Boolean)
					.join(' ');
				return (
					<div
						key={item.id}
						ref={(el) => {
							itemRefs.current[index] = el;
						}}
						className={classes}
						role={role}
						tabIndex={-1}
						aria-disabled={item.disabled || undefined}
						aria-checked={item.type === 'checkbox' ? item.checked : undefined}
						aria-haspopup={isSubmenu ? 'menu' : undefined}
						aria-expanded={isSubmenu ? openSub?.index === index : undefined}
						data-danger={danger || undefined}
						onMouseEnter={() => onItemEnter(index, item)}
						onClick={() => activate(index, false)}
					>
						<span className={styles.icon} aria-hidden="true">
							{isChecked ? <CheckIcon /> : item.icon}
						</span>
						<span className={styles.label}>{item.label}</span>
						{'shortcut' in item && item.shortcut ? (
							<span className={styles.shortcut}>{item.shortcut}</span>
						) : null}
						{isSubmenu ? <ChevronRightIcon className={styles.chevron} /> : null}
					</div>
				);
			})}
			{openSub && submenuItems ? (
				<MenuPanel
					items={submenuItems}
					placement={{ kind: 'anchor', rect: openSub.rect }}
					ariaLabel={openItem && 'label' in openItem ? openItem.label : undefined}
					focusTarget={openSub.viaKeyboard ? 'first' : 'none'}
					nested
					onSelect={onSelect}
					onRequestClose={() => closeSubmenu(true)}
					onDismiss={onDismiss}
				/>
			) : null}
		</div>
	);
}
