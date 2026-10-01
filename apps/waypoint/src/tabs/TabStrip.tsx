// The tab strip: tabs to open, close, reorder and scroll, as a tablist with roving focus
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ClosedTab } from '@liminal-hq/waypoint-protocol/generated/ClosedTab';
import type { MenuPosition } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import {
	useCallback,
	useEffect,
	useLayoutEffect,
	useRef,
	useState,
	type CSSProperties,
	type KeyboardEvent,
	type MouseEvent,
	type PointerEvent,
} from 'react';
import { t, tf } from '../i18n/messages';
import {
	ChevronLeftIcon,
	ChevronRightSmallIcon,
	CloseSmallIcon,
	FolderTabIcon,
	PinIcon,
	PlusIcon,
} from '../icons/AppIcons';
import { announce, clearAnnouncement, useAnnouncement } from './announcer';
import { clampToZone, dropIndex, shiftFor, type Span } from './reorder';
import { colourMessageId } from './tabColours';
import { tabDomId, TAB_PANEL_ID } from './tabIds';
import { useTabsSnapshot } from './TabsContext';
import { useTabActions } from './tabActions';
import { PlusMenu, TabContextMenu } from './TabMenus';
import { TabSwitcher } from './TabSwitcher';
import { useTabTitle } from './tabTitle';
import { useClosedTabs } from './useClosedTabs';
import styles from './TabStrip.module.css';

/** The pointer has to move this far before a press on a tab becomes a drag. */
const DRAG_THRESHOLD_PX = 5;

/** A pinned tab's width plus the gap after it: where the next pinned tab sticks, and the scroll padding. */
const PINNED_STRIDE_PX = 38;

/** How long the + button is held before its menu opens. */
export const PLUS_HOLD_MS = 500;

/** A menu key press also raises a `contextmenu` event in some webviews; the second is ignored. */
const KEYBOARD_MENU_DEBOUNCE_MS = 150;

type MenuState =
	| {
			kind: 'tab';
			tab: TabId;
			position: MenuPosition;
			keyboard: boolean;
			returnTo: HTMLElement | null;
	  }
	| { kind: 'plus'; position: MenuPosition; keyboard: boolean; returnTo: HTMLElement | null };

interface DragState {
	id: TabId;
	from: number;
	startX: number;
	dx: number;
	dragging: boolean;
	spans: Span[];
	to: number;
}

export function TabStrip() {
	const snapshot = useTabsSnapshot();
	const actions = useTabActions();
	const tabs = snapshot?.tabs ?? [];
	const active = snapshot?.active ?? null;

	const scroller = useRef<HTMLDivElement | null>(null);
	const [overflow, setOverflow] = useState({ left: false, right: false });
	const [focused, setFocused] = useState<TabId | null>(null);
	const [drag, setDragState] = useState<DragState | null>(null);
	// The latest drag, for handlers that can run before React has rendered the last update.
	const dragRef = useRef<DragState | null>(null);
	const setDrag = (next: DragState | null) => {
		dragRef.current = next;
		setDragState(next);
	};
	const announcement = useAnnouncement();
	const suppressClick = useRef(false);
	const pinnedCount = tabs.filter((tab) => tab.pinned).length;

	const { closed, refresh } = useClosedTabs();
	const [menu, setMenu] = useState<MenuState | null>(null);
	const menuRequest = useRef(0);
	const lastKeyboardMenu = useRef(0);
	const plusButton = useRef<HTMLButtonElement | null>(null);
	const holdTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
	const heldOpen = useRef(false);

	// The closed list is read fresh each time a menu opens; closing a tab sends no event for it.
	const openMenu = (next: MenuState) => {
		const request = ++menuRequest.current;
		void refresh()
			.catch((): ClosedTab[] => [])
			.then(() => {
				if (request === menuRequest.current) setMenu(next);
			});
	};
	const closeMenu = () => {
		menuRequest.current++;
		setMenu(null);
	};
	useEffect(() => {
		clearAnnouncement();
		return () => clearTimeout(holdTimer.current);
	}, []);

	// A tab made active by any other route (Ctrl+Tab, Alt+digit, a click) takes the focus stop back.
	useEffect(() => setFocused(null), [active]);

	// Roving focus follows the active tab unless the person has moved it with the arrow keys.
	const tabStop = tabs.some((tab) => tab.id === focused) ? focused : active;

	const measureOverflow = useCallback(() => {
		const element = scroller.current;
		if (!element) return;
		const max = element.scrollWidth - element.clientWidth;
		const left = element.scrollLeft > 1;
		const right = element.scrollLeft < max - 1;
		setOverflow((previous) =>
			previous.left === left && previous.right === right ? previous : { left, right },
		);
	}, []);

	useLayoutEffect(measureOverflow, [measureOverflow, tabs.length]);
	useEffect(() => {
		const element = scroller.current;
		if (!element || typeof ResizeObserver === 'undefined') return;
		const observer = new ResizeObserver(measureOverflow);
		observer.observe(element);
		return () => observer.disconnect();
	}, [measureOverflow]);

	// The active tab is never scrolled out of sight.
	useEffect(() => {
		if (active === null) return;
		document
			.getElementById(tabDomId(active))
			?.scrollIntoView?.({ block: 'nearest', inline: 'nearest' });
	}, [active]);

	const scrollBy = (direction: -1 | 1) => {
		const element = scroller.current;
		if (!element) return;
		const left = element.scrollLeft + direction * Math.max(120, element.clientWidth * 0.6);
		if (typeof element.scrollTo === 'function') element.scrollTo({ left, behavior: 'smooth' });
		else element.scrollLeft = left;
	};

	const focusTab = (id: TabId) => {
		setFocused(id);
		document.getElementById(tabDomId(id))?.focus();
	};

	const onTabKeyDown = (event: KeyboardEvent, tab: TabSnapshot, index: number) => {
		const last = tabs.length - 1;
		const step = (target: number) => {
			event.preventDefault();
			const next = tabs[Math.max(0, Math.min(last, target))];
			if (next) focusTab(next.id);
		};
		switch (event.key) {
			case 'ArrowRight':
			case 'ArrowLeft': {
				const delta = event.key === 'ArrowRight' ? 1 : -1;
				if (event.ctrlKey && event.shiftKey) {
					// Keyboard reorder, the counterpart of dragging.
					event.preventDefault();
					// A tab stays on its own side of the pinned boundary, as the session store keeps it.
					const target = clampToZone(index + delta, tab.pinned, pinnedCount, tabs.length);
					if (target !== index) {
						actions.move(tab.id, target);
						announce(
							tf('tabs.moved', {
								title: tab.location.display,
								position: target + 1,
								count: tabs.length,
							}),
						);
					}
					return;
				}
				return step(index + delta);
			}
			case 'Home':
				return step(0);
			case 'End':
				return step(last);
			case 'Enter':
			case ' ':
				event.preventDefault();
				return actions.activate(tab.id);
			case 'Delete':
				event.preventDefault();
				return actions.close(tab.id);
			case 'ContextMenu':
			case 'F10': {
				if (event.key === 'F10' && !event.shiftKey) return;
				event.preventDefault();
				const element = event.currentTarget as HTMLElement;
				const rect = element.getBoundingClientRect();
				lastKeyboardMenu.current = Date.now();
				openMenu({
					kind: 'tab',
					tab: tab.id,
					position: { x: rect.left, y: rect.bottom },
					keyboard: true,
					returnTo: element,
				});
				return;
			}
		}
	};

	const onTabContextMenu = (event: MouseEvent, tab: TabSnapshot) => {
		event.preventDefault();
		if (Date.now() - lastKeyboardMenu.current < KEYBOARD_MENU_DEBOUNCE_MS) return;
		openMenu({
			kind: 'tab',
			tab: tab.id,
			position: { x: event.clientX, y: event.clientY },
			keyboard: false,
			returnTo: document.getElementById(tabDomId(tab.id)),
		});
	};

	const openPlusMenu = (position: MenuPosition, keyboard: boolean) =>
		openMenu({ kind: 'plus', position, keyboard, returnTo: plusButton.current });

	const onPointerDown = (event: PointerEvent<HTMLDivElement>, id: TabId, index: number) => {
		if (event.button !== 0 || (event.target as HTMLElement).closest('button')) return;
		const slots = [...(scroller.current?.querySelectorAll<HTMLElement>('[data-slot]') ?? [])];
		const spans = slots.map((slot) => {
			const rect = slot.getBoundingClientRect();
			return { left: rect.left, right: rect.right };
		});
		event.currentTarget.setPointerCapture?.(event.pointerId);
		setDrag({ id, from: index, startX: event.clientX, dx: 0, dragging: false, spans, to: index });
	};

	const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
		const drag = dragRef.current;
		if (!drag) return;
		const dx = event.clientX - drag.startX;
		const dragging = drag.dragging || Math.abs(dx) >= DRAG_THRESHOLD_PX;
		if (!dragging) return;
		const origin = drag.spans[drag.from];
		const centre = origin ? (origin.left + origin.right) / 2 + dx : event.clientX;
		const pinned = tabs[drag.from]?.pinned ?? false;
		const to = clampToZone(
			dropIndex(drag.spans, drag.from, centre),
			pinned,
			pinnedCount,
			tabs.length,
		);
		setDrag({ ...drag, dx, dragging, to });
	};

	const endDrag = (commit: boolean) => {
		const drag = dragRef.current;
		if (drag?.dragging) {
			suppressClick.current = true;
			if (commit && drag.to !== drag.from) actions.move(drag.id, drag.to);
			// The click that ends a drag is not a click on the tab; drop the flag if none follows.
			setTimeout(() => {
				suppressClick.current = false;
			}, 0);
		}
		setDrag(null);
	};

	// Escape abandons a drag in progress.
	useEffect(() => {
		if (!drag?.dragging) return;
		const onKey = (event: globalThis.KeyboardEvent) => {
			if (event.key === 'Escape') {
				event.stopPropagation();
				suppressClick.current = true;
				setDrag(null);
			}
		};
		window.addEventListener('keydown', onKey, true);
		return () => window.removeEventListener('keydown', onKey, true);
	}, [drag?.dragging]);

	const onSlotClick = (id: TabId) => {
		if (suppressClick.current) {
			suppressClick.current = false;
			return;
		}
		actions.activate(id);
	};

	const onAuxClick = (event: MouseEvent, tab: TabSnapshot) => {
		// The close button handles its own middle-click, so a later confirm-on-close cannot be bypassed.
		if (event.button !== 1 || (event.target as HTMLElement).closest('button')) return;
		event.preventDefault();
		// A pinned tab is not closed by an accidental middle-click; the menu and the keys still close it.
		if (!tab.pinned) actions.close(tab.id);
	};

	return (
		<div className={styles.strip}>
			<button
				type="button"
				className={styles.arrow}
				aria-label={t('tabs.scrollLeft')}
				hidden={!overflow.left}
				tabIndex={-1}
				onClick={() => scrollBy(-1)}
			>
				<ChevronLeftIcon />
			</button>
			<div
				ref={scroller}
				className={styles.scroller}
				style={{ '--wp-pinned-width': `${pinnedCount * PINNED_STRIDE_PX}px` } as CSSProperties}
				onScroll={measureOverflow}
				onWheel={(event) => {
					// A vertical wheel scrolls a horizontal strip.
					if (Math.abs(event.deltaY) > Math.abs(event.deltaX) && scroller.current) {
						scroller.current.scrollLeft += event.deltaY;
					}
				}}
			>
				<div role="tablist" aria-label={t('tabs.strip.label')} className={styles.tablist}>
					{tabs.map((tab, index) => {
						const dragged = drag?.dragging && drag.id === tab.id;
						const width = drag
							? (drag.spans[drag.from]?.right ?? 0) - (drag.spans[drag.from]?.left ?? 0)
							: 0;
						const shift = drag?.dragging ? shiftFor(index, drag.from, drag.to, width) : 0;
						return (
							<div
								key={tab.id}
								role="presentation"
								className={styles.slot}
								data-slot=""
								data-index={index}
								data-active={tab.id === active ? '' : undefined}
								data-pinned={tab.pinned ? '' : undefined}
								data-colour={tab.colour ?? undefined}
								data-dragging={dragged ? '' : undefined}
								style={
									{
										'--wp-tab-shift': `${dragged ? (drag?.dx ?? 0) : shift}px`,
										'--wp-pin-index': index,
									} as CSSProperties
								}
								onPointerDown={(event) => onPointerDown(event, tab.id, index)}
								onPointerMove={onPointerMove}
								onPointerUp={() => endDrag(true)}
								onPointerCancel={() => endDrag(false)}
								onMouseDown={(event) => {
									// Stops middle-click from starting the platform's autoscroll.
									if (event.button === 1) event.preventDefault();
								}}
								onAuxClick={(event) => onAuxClick(event, tab)}
								onContextMenu={(event) => onTabContextMenu(event, tab)}
								onClick={() => onSlotClick(tab.id)}
							>
								<TabButton
									tab={tab}
									selected={tab.id === active}
									tabStop={tab.id === tabStop}
									onFocus={() => setFocused(tab.id)}
									onKeyDown={(event) => onTabKeyDown(event, tab, index)}
								/>
								{tab.pinned ? null : (
									<CloseButton tab={tab} onClose={() => actions.close(tab.id)} />
								)}
							</div>
						);
					})}
				</div>
			</div>
			<button
				type="button"
				className={styles.arrow}
				aria-label={t('tabs.scrollRight')}
				hidden={!overflow.right}
				tabIndex={-1}
				onClick={() => scrollBy(1)}
			>
				<ChevronRightSmallIcon />
			</button>
			<button
				type="button"
				ref={plusButton}
				className={styles.plus}
				aria-label={t('tabs.new')}
				aria-haspopup="menu"
				title={t('tabs.new')}
				onClick={() => {
					// The click that ends a press-and-hold is not a request for a new tab.
					if (heldOpen.current) heldOpen.current = false;
					else actions.newTab();
				}}
				onPointerDown={(event) => {
					if (event.button !== 0) return;
					heldOpen.current = false;
					clearTimeout(holdTimer.current);
					const { clientX, clientY } = event;
					holdTimer.current = setTimeout(() => {
						heldOpen.current = true;
						openPlusMenu({ x: clientX, y: clientY }, false);
					}, PLUS_HOLD_MS);
				}}
				onPointerUp={() => clearTimeout(holdTimer.current)}
				onPointerLeave={() => clearTimeout(holdTimer.current)}
				onPointerCancel={() => clearTimeout(holdTimer.current)}
				onContextMenu={(event) => {
					event.preventDefault();
					if (Date.now() - lastKeyboardMenu.current < KEYBOARD_MENU_DEBOUNCE_MS) return;
					clearTimeout(holdTimer.current);
					openPlusMenu({ x: event.clientX, y: event.clientY }, false);
				}}
				onKeyDown={(event) => {
					if (event.key !== 'ContextMenu' && !(event.key === 'F10' && event.shiftKey)) return;
					event.preventDefault();
					const rect = event.currentTarget.getBoundingClientRect();
					lastKeyboardMenu.current = Date.now();
					openPlusMenu({ x: rect.left, y: rect.bottom }, true);
				}}
				onMouseDown={(event) => {
					if (event.button === 1) event.preventDefault();
				}}
				onAuxClick={(event) => {
					if (event.button === 1) {
						event.preventDefault();
						actions.newTabAtHome();
					}
				}}
			>
				<PlusIcon />
			</button>
			<div className={styles.srOnly} role="status" aria-live="polite">
				{announcement}
			</div>
			<TabSwitcher />
			{menu?.kind === 'tab' && tabs.some((tab) => tab.id === menu.tab) ? (
				<TabContextMenu
					tab={tabs.find((tab) => tab.id === menu.tab)!}
					closed={closed}
					position={menu.position}
					openedWithKeyboard={menu.keyboard}
					returnFocusTo={menu.returnTo}
					onClose={closeMenu}
				/>
			) : null}
			{menu?.kind === 'plus' ? (
				<PlusMenu
					closed={closed}
					position={menu.position}
					openedWithKeyboard={menu.keyboard}
					returnFocusTo={menu.returnTo}
					onClose={closeMenu}
				/>
			) : null}
		</div>
	);
}

interface TabButtonProps {
	tab: TabSnapshot;
	selected: boolean;
	tabStop: boolean;
	onFocus: () => void;
	onKeyDown: (event: KeyboardEvent) => void;
}

function TabButton({ tab, selected, tabStop, onFocus, onKeyDown }: TabButtonProps) {
	const title = useTabTitle(tab);
	// The colour and the pin are words as well as marks, so neither is conveyed by appearance alone.
	const details = [
		tab.pinned ? t('tabs.pinned') : null,
		tab.colour ? tf('tabs.colourDescription', { colour: t(colourMessageId(tab.colour)) }) : null,
	].filter((detail) => detail !== null);
	const describedBy = details.length > 0 ? `${tabDomId(tab.id)}-details` : undefined;
	return (
		<>
			<div
				id={tabDomId(tab.id)}
				role="tab"
				className={styles.tab}
				aria-selected={selected}
				aria-controls={selected ? TAB_PANEL_ID : undefined}
				tabIndex={tabStop ? 0 : -1}
				// A pinned tab shows no text, so its name is stated.
				aria-label={tab.pinned ? title : undefined}
				aria-describedby={describedBy}
				title={[tab.location.display, ...details].join(' · ')}
				onFocus={onFocus}
				onKeyDown={onKeyDown}
			>
				<FolderTabIcon className={styles.icon} />
				{tab.pinned ? <PinIcon className={styles.pinBadge} width={10} height={10} /> : null}
				{tab.pinned ? null : <span className={styles.title}>{title}</span>}
			</div>
			{describedBy ? (
				<span id={describedBy} hidden>
					{details.join('. ')}
				</span>
			) : null}
		</>
	);
}

function CloseButton({ tab, onClose }: { tab: TabSnapshot; onClose: () => void }) {
	const title = useTabTitle(tab);
	return (
		<button
			type="button"
			className={styles.close}
			tabIndex={-1}
			aria-label={tf('tabs.close', { title })}
			title={tf('tabs.close', { title })}
			onClick={(event) => {
				event.stopPropagation();
				onClose();
			}}
			onPointerDown={(event) => event.stopPropagation()}
			onAuxClick={(event) => {
				if (event.button !== 1) return;
				event.preventDefault();
				event.stopPropagation();
				onClose();
			}}
		>
			<CloseSmallIcon width={12} height={12} />
		</button>
	);
}
