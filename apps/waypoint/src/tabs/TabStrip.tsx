// The tab strip: tabs to open, close, reorder and scroll, as a tablist with roving focus
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ClosedTab } from '@liminal-hq/waypoint-protocol/generated/ClosedTab';
import type { MenuPosition } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { GroupId } from '@liminal-hq/waypoint-protocol/generated/GroupId';
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
import { dropIndex, shiftFor, type Span } from './reorder';
import { chipDomId, GroupChip } from './GroupChip';
import { GroupMenu } from './GroupMenu';
import {
	requestRename,
	useExpandOnActivate,
	useGroupActions,
	useGroupLimitWarning,
	useRenaming,
} from './groupActions';
import {
	buildStrip,
	hiddenActiveGroup,
	landingIndex,
	measureSpans,
	stepTarget,
	type StripItem,
} from './groupLayout';
import { colourMessageId } from './tabColours';
import { pairOfTab } from './pairLayout';
import { PairJoint, pairName, pairSlotAttributes } from './PairPill';
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
	| {
			kind: 'group';
			group: GroupId;
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
	const layout = buildStrip(snapshot);
	const groupActions = useGroupActions();
	const renaming = useRenaming();
	const limitWarning = useGroupLimitWarning();
	useExpandOnActivate();
	// A group chip is a stop in the roving order too; `focusedChip` is set while one holds focus.
	const [focusedChip, setFocusedChip] = useState<GroupId | null>(null);
	const hiddenGroup = hiddenActiveGroup(snapshot);
	// A paired tab says which split it is in, so the pill's name reaches each half.
	const pairLabelFor = (id: TabId): string | undefined => {
		const pair = pairOfTab(snapshot?.pairs ?? [], id);
		return pair ? pairName(pair, snapshot) : undefined;
	};

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
	useEffect(() => {
		setFocused(null);
		setFocusedChip(null);
	}, [active]);

	// Roving focus follows the active tab unless the person has moved it with the arrow keys. When
	// the active tab is hidden in a collapsed group, its chip is the stop.
	const visibleTab = (id: TabId | null) =>
		layout.items.some((item) => item.kind === 'tab' && item.tab.id === id);
	const chipStop = layout.items.some(
		(item) => item.kind === 'chip' && item.group.id === focusedChip,
	)
		? focusedChip
		: visibleTab(focused)
			? null
			: hiddenGroup;
	const tabStop = chipStop !== null ? null : visibleTab(focused) ? focused : active;

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

	useLayoutEffect(measureOverflow, [measureOverflow, tabs.length, layout.items.length]);
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
		setFocusedChip(null);
		document.getElementById(tabDomId(id))?.focus();
	};

	const focusItem = (item: StripItem | undefined) => {
		if (!item) return;
		if (item.kind === 'tab') return focusTab(item.tab.id);
		setFocusedChip(item.group.id);
		document.getElementById(chipDomId(item.group.id))?.focus();
	};

	/** Arrow, Home and End keys move through the chips and visible tabs alike. */
	const navigate = (event: KeyboardEvent, at: number): boolean => {
		const last = layout.items.length - 1;
		const go = (target: number) => {
			event.preventDefault();
			focusItem(layout.items[Math.max(0, Math.min(last, target))]);
			return true;
		};
		switch (event.key) {
			case 'ArrowRight':
				return go(at + 1);
			case 'ArrowLeft':
				return go(at - 1);
			case 'Home':
				return go(0);
			case 'End':
				return go(last);
		}
		return false;
	};

	const onChipKeyDown = (event: KeyboardEvent, item: StripItem & { kind: 'chip' }, at: number) => {
		if (
			(event.key === 'ArrowRight' || event.key === 'ArrowLeft') &&
			event.ctrlKey &&
			event.shiftKey
		) {
			// Keyboard move of the whole group, the counterpart of dragging its chip.
			event.preventDefault();
			groupActions.moveBy(item.group, event.key === 'ArrowRight' ? 1 : -1);
			return;
		}
		if (navigate(event, at)) return;
		switch (event.key) {
			case 'Enter':
			case ' ':
				event.preventDefault();
				return groupActions.setCollapsed(item.group, !item.group.collapsed);
			case 'ContextMenu':
			case 'F10': {
				if (event.key === 'F10' && !event.shiftKey) return;
				event.preventDefault();
				const element = event.currentTarget as HTMLElement;
				const rect = element.getBoundingClientRect();
				lastKeyboardMenu.current = Date.now();
				openMenu({
					kind: 'group',
					group: item.group.id,
					position: { x: rect.left, y: rect.bottom },
					keyboard: true,
					returnTo: element,
				});
				return;
			}
		}
	};

	const onChipContextMenu = (event: MouseEvent, item: StripItem & { kind: 'chip' }) => {
		event.preventDefault();
		if (Date.now() - lastKeyboardMenu.current < KEYBOARD_MENU_DEBOUNCE_MS) return;
		openMenu({
			kind: 'group',
			group: item.group.id,
			position: { x: event.clientX, y: event.clientY },
			keyboard: false,
			returnTo: document.getElementById(chipDomId(item.group.id)),
		});
	};

	const onTabKeyDown = (event: KeyboardEvent, tab: TabSnapshot, index: number) => {
		const at = layout.items.findIndex((item) => item.kind === 'tab' && item.tab.id === tab.id);
		switch (event.key) {
			case 'ArrowRight':
			case 'ArrowLeft': {
				const delta = event.key === 'ArrowRight' ? 1 : -1;
				if (event.ctrlKey && event.shiftKey) {
					// Keyboard reorder, the counterpart of dragging.
					event.preventDefault();
					// The session keeps a tab on its own side of the pinned boundary and a grouped tab
					// inside its group; a tab outside groups steps over a neighbouring group whole.
					const target = stepTarget(tabs, index, delta);
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
				return void navigate(event, at);
			}
			case 'Home':
			case 'End':
				return void navigate(event, at);
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
		const spans = scroller.current ? measureSpans(scroller.current, tabs) : [];
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
		// Where the session will really leave the tab: it stays on its side of the pinned boundary,
		// a grouped tab stays in its group, and a drop into another group's run lands beside the group.
		const to = landingIndex(tabs, drag.from, dropIndex(drag.spans, drag.from, centre));
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
				style={{ '--wp-pinned-width': `${layout.pinSlots * PINNED_STRIDE_PX}px` } as CSSProperties}
				onScroll={measureOverflow}
				onWheel={(event) => {
					// A vertical wheel scrolls a horizontal strip.
					if (Math.abs(event.deltaY) > Math.abs(event.deltaX) && scroller.current) {
						scroller.current.scrollLeft += event.deltaY;
					}
				}}
			>
				<div role="tablist" aria-label={t('tabs.strip.label')} className={styles.tablist}>
					{layout.items.map((item, at) => {
						if (item.kind === 'chip') {
							const chipDragShift =
								drag?.dragging && drag.from !== item.firstIndex
									? shiftFor(
											item.firstIndex,
											drag.from,
											drag.to,
											(drag.spans[drag.from]?.right ?? 0) - (drag.spans[drag.from]?.left ?? 0),
										)
									: 0;
							return (
								<GroupChip
									key={`group-${item.group.id}`}
									item={item}
									shift={chipDragShift}
									tabStop={chipStop === item.group.id}
									renaming={renaming === item.group.id}
									onFocus={() => setFocusedChip(item.group.id)}
									onSetCollapsed={(collapsed) => groupActions.setCollapsed(item.group, collapsed)}
									onRename={(name) => groupActions.rename(item.group, name)}
									onStartRename={() => requestRename(item.group.id)}
									onKeyDown={(event) => onChipKeyDown(event, item, at)}
									onContextMenu={(event) => onChipContextMenu(event, item)}
								/>
							);
						}
						const { tab, index } = item;
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
								data-group={item.group ? item.group.id : undefined}
								data-group-first={item.groupFirst ? '' : undefined}
								data-group-last={item.groupLast ? '' : undefined}
								data-dragging={dragged ? '' : undefined}
								{...pairSlotAttributes(snapshot, tab.id)}
								style={
									{
										'--wp-tab-shift': `${dragged ? (drag?.dx ?? 0) : shift}px`,
										'--wp-pin-index': item.pinIndex,
										'--wp-group-accent': `var(--wp-tab-colour-${item.group?.colour ?? 'grey'})`,
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
									groupName={item.group?.name}
									selected={tab.id === active}
									tabStop={tab.id === tabStop}
									pairLabel={pairLabelFor(tab.id)}
									onFocus={() => setFocused(tab.id)}
									onKeyDown={(event) => onTabKeyDown(event, tab, index)}
								/>
								{tab.pinned ? null : (
									<CloseButton tab={tab} onClose={() => actions.close(tab.id)} />
								)}
								{pairOfTab(snapshot?.pairs ?? [], tab.id) ? (
									<PairJoint
										pair={pairOfTab(snapshot?.pairs ?? [], tab.id)!}
										snapshot={snapshot}
										tab={tab.id}
									/>
								) : null}
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
				onPointerUp={() => {
					clearTimeout(holdTimer.current);
					// The click that ends a hold follows in the same task; one that never comes (the
					// pointer was released elsewhere) must not swallow the next keyboard activation.
					setTimeout(() => (heldOpen.current = false), 0);
				}}
				onPointerLeave={() => {
					clearTimeout(holdTimer.current);
					heldOpen.current = false;
				}}
				onPointerCancel={() => {
					clearTimeout(holdTimer.current);
					heldOpen.current = false;
				}}
				onBlur={() => (heldOpen.current = false)}
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
			{limitWarning ? (
				<div className={styles.warning} role="note">
					{limitWarning.text}
				</div>
			) : null}
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
			{menu?.kind === 'group' ? (
				<GroupMenuFor menu={menu} items={layout.items} onClose={closeMenu} />
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

/** The group menu for the group `menu` names, if it is still there. */
function GroupMenuFor({
	menu,
	items,
	onClose,
}: {
	menu: Extract<MenuState, { kind: 'group' }>;
	items: StripItem[];
	onClose: () => void;
}) {
	const chip = items.find((item) => item.kind === 'chip' && item.group.id === menu.group);
	if (chip?.kind !== 'chip') return null;
	const others = items.some((item) => item.kind === 'chip' && item.group.id !== menu.group);
	return (
		<GroupMenu
			group={chip.group}
			pinned={chip.pinned}
			hasOthers={others}
			position={menu.position}
			openedWithKeyboard={menu.keyboard}
			returnFocusTo={menu.returnTo}
			onClose={onClose}
			// The menu returns focus to the chip as it closes; the name field takes it after that.
			onRename={() => setTimeout(() => requestRename(menu.group), 0)}
		/>
	);
}

interface TabButtonProps {
	tab: TabSnapshot;
	groupName: string | undefined;
	selected: boolean;
	tabStop: boolean;
	/** "Split: A and B" when the tab is half of a pair. */
	pairLabel: string | undefined;
	onFocus: () => void;
	onKeyDown: (event: KeyboardEvent) => void;
}

function TabButton({
	tab,
	groupName,
	selected,
	tabStop,
	pairLabel,
	onFocus,
	onKeyDown,
}: TabButtonProps) {
	const title = useTabTitle(tab);
	// The colour and the pin are words as well as marks, so neither is conveyed by appearance alone.
	const details = [
		pairLabel ?? null,
		groupName === undefined ? null : tf('groups.tab.member', { name: groupName }),
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
