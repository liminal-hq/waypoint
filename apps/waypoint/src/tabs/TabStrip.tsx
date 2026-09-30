// The tab strip: tabs to open, close, reorder and scroll, as a tablist with roving focus
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

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
import { useVfsClient } from '../browse/VfsClientContext';
import { t, tf } from '../i18n/messages';
import {
	ChevronLeftIcon,
	ChevronRightSmallIcon,
	CloseSmallIcon,
	FolderTabIcon,
	PlusIcon,
} from '../icons/AppIcons';
import { useLocationInfo } from '../nav/locationInfo';
import { dropIndex, shiftFor, type Span } from './reorder';
import { tabDomId, TAB_PANEL_ID } from './tabIds';
import { useTabsSnapshot } from './TabsContext';
import { useTabActions } from './tabActions';
import styles from './TabStrip.module.css';

/** The pointer has to move this far before a press on a tab becomes a drag. */
const DRAG_THRESHOLD_PX = 5;

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
	const [announcement, setAnnouncement] = useState('');
	const suppressClick = useRef(false);

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
					const target = Math.max(0, Math.min(last, index + delta));
					if (target !== index) {
						actions.move(tab.id, target);
						setAnnouncement(
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
		}
	};

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
		setDrag({ ...drag, dx, dragging, to: dropIndex(drag.spans, drag.from, centre) });
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

	const onAuxClick = (event: MouseEvent, id: TabId) => {
		// The close button handles its own middle-click, so a later confirm-on-close cannot be bypassed.
		if (event.button !== 1 || (event.target as HTMLElement).closest('button')) return;
		event.preventDefault();
		actions.close(id);
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
								data-dragging={dragged ? '' : undefined}
								style={
									{
										'--wp-tab-shift': `${dragged ? (drag?.dx ?? 0) : shift}px`,
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
								onAuxClick={(event) => onAuxClick(event, tab.id)}
								onClick={() => onSlotClick(tab.id)}
							>
								<TabButton
									tab={tab}
									selected={tab.id === active}
									tabStop={tab.id === tabStop}
									onFocus={() => setFocused(tab.id)}
									onKeyDown={(event) => onTabKeyDown(event, tab, index)}
								/>
								<CloseButton tab={tab} onClose={() => actions.close(tab.id)} />
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
				className={styles.plus}
				aria-label={t('tabs.new')}
				title={t('tabs.new')}
				onClick={actions.newTab}
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

function useTabTitle(tab: TabSnapshot): string {
	const info = useLocationInfo(useVfsClient(), tab.location);
	// Until Rust has answered, the full display path stands in for the folder name.
	return info?.segments[info.segments.length - 1]?.label ?? tab.location.display;
}

function TabButton({ tab, selected, tabStop, onFocus, onKeyDown }: TabButtonProps) {
	const title = useTabTitle(tab);
	return (
		<div
			id={tabDomId(tab.id)}
			role="tab"
			className={styles.tab}
			aria-selected={selected}
			aria-controls={selected ? TAB_PANEL_ID : undefined}
			tabIndex={tabStop ? 0 : -1}
			title={tab.location.display}
			onFocus={onFocus}
			onKeyDown={onKeyDown}
		>
			<FolderTabIcon className={styles.icon} />
			<span className={styles.title}>{title}</span>
		</div>
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
