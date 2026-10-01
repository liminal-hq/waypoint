// The Action bar (SPEC 5.9): the common file commands in a row under the toolbar
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ContextMenu } from '@liminal-hq/waypoint-chrome/ContextMenu';
import type { MenuItem, MenuPosition } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import {
	useCallback,
	useId,
	useLayoutEffect,
	useMemo,
	useRef,
	useState,
	type KeyboardEvent,
	type MouseEvent,
} from 'react';
import { useCommandBridge, useCommands } from '../commands/commandBridge';
import type { CommandId } from '../commands/registry';
import { t } from '../i18n/messages';
import { ChevronRightSmallIcon } from '../icons/AppIcons';
import { ActionBarIcon, MoreIcon, TextIcon } from '../icons/MenuIcons';
import { actionBarItems, dividerBefore, overflowRows, tooltipFor } from './actionBarModel';
import { fitCount } from './actionBarOverflow';
import styles from './ActionBar.module.css';

type OpenMenu =
	| {
			kind: 'item';
			id: string;
			items: MenuItem[];
			position: MenuPosition;
			keyboard: boolean;
			anchor: HTMLElement | null;
	  }
	| { kind: 'bar'; position: MenuPosition; keyboard: boolean };

/** The size of the More button before it has been drawn, so the first measure leaves room for it. */
const MORE_WIDTH_GUESS = 36;

const px = (value: string) => Number.parseFloat(value) || 0;

/**
 * A row of buttons that are registry commands bound to the active pane: each is enabled or
 * disabled by the command's availability (a disabled button stays in the focus order and says why
 * in its tooltip), and each runs through `run`, the same path as the menu and the keys. The bar
 * is a toolbar with one tab stop: Left, Right, Home and End move between buttons. Trailing buttons
 * that do not fit collapse into a More menu. It shows only when the Action bar setting is on
 * (View → Action Bar) and, being part of the workspace, only in Main windows.
 */
export function ActionBar() {
	const api = useCommands();
	const bridge = useCommandBridge();
	const { facts, run } = api;
	const items = useMemo(() => actionBarItems(api), [api]);
	const labels = facts.actionBarLabels;
	const barRef = useRef<HTMLDivElement>(null);
	const cellRefs = useRef<Array<HTMLDivElement | null>>([]);
	const buttonRefs = useRef<Array<HTMLButtonElement | null>>([]);
	const moreRef = useRef<HTMLDivElement>(null);
	const hintId = useId();
	const [shown, setShown] = useState(Number.POSITIVE_INFINITY);
	const [focusIndex, setFocusIndex] = useState(0);
	const [menu, setMenu] = useState<OpenMenu | null>(null);

	const visible = Math.min(shown, items.length);
	const overflowed = items.slice(visible);
	// The focus order: the buttons on show, then More.
	const stops = visible + (overflowed.length > 0 ? 1 : 0);
	const stop = Math.min(focusIndex, Math.max(stops - 1, 0));

	// Measures the buttons (hidden ones included, which stay in the document) and decides how many fit.
	const signature = items.map((item) => `${item.id}:${item.label}`).join('|') + String(labels);
	const barShown = facts.actionBar;
	const measure = useCallback(() => {
		const bar = barRef.current;
		if (!bar) return;
		const widths = items.map((_, index) => cellRefs.current[index]?.offsetWidth ?? 0);
		// Not laid out (hidden, or no layout engine): nothing to decide, show everything.
		if (widths.every((width) => width === 0)) return;
		const style = getComputedStyle(bar);
		const available = bar.clientWidth - px(style.paddingLeft) - px(style.paddingRight);
		const more = moreRef.current?.offsetWidth || MORE_WIDTH_GUESS;
		const count = fitCount(widths, available, px(style.columnGap), more);
		setShown((previous) => (Math.min(previous, items.length) === count ? previous : count));
	}, [items]);
	useLayoutEffect(() => {
		if (!barShown) return;
		measure();
		const bar = barRef.current;
		if (!bar || typeof ResizeObserver === 'undefined') return;
		const observer = new ResizeObserver(() => measure());
		observer.observe(bar);
		return () => observer.disconnect();
		// `signature` stands for "the buttons or their labels changed".
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [measure, signature, barShown]);

	if (!facts.actionBar) return null;

	const focusStop = (index: number) => {
		setFocusIndex(index);
		const target = index < visible ? buttonRefs.current[index] : buttonRefs.current[items.length];
		target?.focus();
	};

	const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
		const active = buttonRefs.current.findIndex((button) => button === document.activeElement);
		const current = active === items.length ? visible : active;
		if (current < 0 || menu) return;
		switch (event.key) {
			case 'ArrowRight':
				event.preventDefault();
				if (current < stops - 1) focusStop(current + 1);
				return;
			case 'ArrowLeft':
				event.preventDefault();
				if (current > 0) focusStop(current - 1);
				return;
			case 'Home':
				event.preventDefault();
				focusStop(0);
				return;
			case 'End':
				event.preventDefault();
				focusStop(stops - 1);
				return;
			case 'ContextMenu':
			case 'F10':
				if (event.key === 'F10' && !event.shiftKey) return;
				event.preventDefault();
				openBarMenu(buttonRefs.current[active] ?? null, true);
		}
	};

	const anchorOf = (button: HTMLElement | null): MenuPosition => {
		const box = button?.getBoundingClientRect();
		return { x: box?.left ?? 0, y: box?.bottom ?? 0 };
	};

	const openBarMenu = (anchor: HTMLElement | null, keyboard: boolean, at?: MenuPosition) =>
		setMenu({ kind: 'bar', position: at ?? anchorOf(anchor), keyboard });

	const openItemMenu = (
		id: string,
		rows: MenuItem[],
		button: HTMLElement | null,
		keyboard: boolean,
	) =>
		setMenu({
			kind: 'item',
			id,
			items: rows,
			position: anchorOf(button),
			keyboard,
			anchor: button,
		});

	const onContextMenu = (event: MouseEvent<HTMLDivElement>) => {
		event.preventDefault();
		openBarMenu(null, false, { x: event.clientX, y: event.clientY });
	};

	const barRows: MenuItem[] = [
		{
			type: 'action',
			id: 'labels',
			label: t(labels ? 'actionBar.hideLabels' : 'actionBar.showLabels'),
			icon: <TextIcon />,
		},
		{ type: 'action', id: 'hide', label: t('actionBar.hide'), icon: <ActionBarIcon /> },
	];

	const onSelect = (item: { id: string }) => {
		if (menu?.kind === 'bar') {
			const { actions } = bridge.store.getState();
			if (item.id === 'labels') actions.setActionBarLabels(!labels);
			else if (item.id === 'hide') actions.setActionBar(false);
			return;
		}
		run(item.id as CommandId);
	};

	return (
		<>
			<div
				ref={barRef}
				className={styles.bar}
				role="toolbar"
				aria-label={t('actionBar.label')}
				aria-orientation="horizontal"
				data-labels={labels}
				onKeyDown={onKeyDown}
				onContextMenu={onContextMenu}
			>
				{items.map((item, index) => {
					const Icon = item.icon;
					const hintFor = item.enabled || !item.reason ? undefined : `${hintId}-${item.id}`;
					const isMenu = item.menu !== undefined;
					const onBar = index < visible;
					return (
						<div
							key={item.id}
							ref={(element) => {
								cellRefs.current[index] = element;
							}}
							className={styles.cell}
							data-divider={dividerBefore(items, index) || undefined}
							data-overflow={onBar ? undefined : ''}
						>
							<button
								ref={(element) => {
									buttonRefs.current[index] = element;
								}}
								type="button"
								className={styles.button}
								data-action={item.id}
								aria-label={item.label}
								aria-disabled={!item.enabled || undefined}
								aria-describedby={hintFor}
								aria-haspopup={isMenu ? 'menu' : undefined}
								aria-expanded={isMenu ? menu?.kind === 'item' && menu.id === item.id : undefined}
								title={item.tooltip}
								tabIndex={onBar && stop === index ? 0 : -1}
								onFocus={() => onBar && setFocusIndex(index)}
								onClick={(event) => {
									if (!item.enabled) return;
									if (item.menu)
										openItemMenu(item.id, item.menu, event.currentTarget, event.detail === 0);
									else if (item.command) run(item.command);
								}}
								onKeyDown={(event) => {
									if (item.menu && item.enabled && event.key === 'ArrowDown') {
										event.preventDefault();
										openItemMenu(item.id, item.menu, event.currentTarget, true);
									}
								}}
							>
								<Icon />
								{labels && <span className={styles.label}>{item.label}</span>}
								{isMenu && (
									<ChevronRightSmallIcon
										className={styles.chevron}
										style={{ transform: 'rotate(90deg)' }}
									/>
								)}
							</button>
							{hintFor && (
								<span id={hintFor} className={styles.hint}>
									{item.reason}
								</span>
							)}
						</div>
					);
				})}
				{overflowed.length > 0 && (
					<div ref={moreRef} className={styles.cell}>
						<button
							ref={(element) => {
								buttonRefs.current[items.length] = element;
							}}
							type="button"
							className={styles.button}
							data-action="more"
							aria-label={t('actionBar.more')}
							aria-haspopup="menu"
							aria-expanded={menu?.kind === 'item' && menu.id === 'more'}
							title={tooltipFor(t('actionBar.more'), undefined, undefined)}
							tabIndex={stop === visible ? 0 : -1}
							onFocus={() => setFocusIndex(visible)}
							onClick={(event) =>
								openItemMenu(
									'more',
									overflowRows(overflowed),
									event.currentTarget,
									event.detail === 0,
								)
							}
							onKeyDown={(event) => {
								if (event.key === 'ArrowDown') {
									event.preventDefault();
									openItemMenu('more', overflowRows(overflowed), event.currentTarget, true);
								}
							}}
						>
							<MoreIcon />
						</button>
					</div>
				)}
			</div>
			{menu && (
				<ContextMenu
					items={menu.kind === 'bar' ? barRows : menu.items}
					position={menu.position}
					ariaLabel={t(menu.kind === 'bar' ? 'actionBar.menu.label' : 'actionBar.label')}
					returnFocusTo={menu.kind === 'item' ? menu.anchor : undefined}
					openedWithKeyboard={menu.keyboard}
					onSelect={onSelect}
					onClose={() => setMenu(null)}
				/>
			)}
		</>
	);
}
