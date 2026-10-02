// The toolbar's panel toggles (sidebar, split view, Shelf), or a More menu with them when the toolbar is narrow
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ContextMenu } from '@liminal-hq/waypoint-chrome/ContextMenu';
import type { MenuItem, MenuPosition } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import { createElement, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { useCommands } from '../commands/commandBridge';
import { t } from '../i18n/messages';
import { MoreIcon } from '../icons/MenuIcons';
import { NavButton } from './NavButton';
import buttonStyles from './NavButton.module.css';
import { panelToggles, shouldCollapse } from './panelToggles';
import styles from './PanelToggles.module.css';

/**
 * Toggle buttons for the panels, each the registry command of the same name: pressed is the
 * command's `checked` and a press is `run(id)`, the path the menu, the palette and the keys take.
 * Below `COLLAPSE_BELOW` (measured on the toolbar that holds them) they become one More button
 * whose menu lists the same commands with their state.
 */
export function PanelToggles() {
	const api = useCommands();
	const { run } = api;
	const toggles = useMemo(() => panelToggles(api), [api]);
	const groupRef = useRef<HTMLDivElement>(null);
	const moreRef = useRef<HTMLButtonElement>(null);
	const [collapsed, setCollapsed] = useState(false);
	const [menu, setMenu] = useState<{ position: MenuPosition; keyboard: boolean } | null>(null);

	// The toolbar is the group's parent: its width, not the group's, says whether there is room.
	useLayoutEffect(() => {
		const bar = groupRef.current?.parentElement;
		if (!bar) return;
		const measure = () => setCollapsed(shouldCollapse(bar.clientWidth));
		measure();
		if (typeof ResizeObserver === 'undefined') return;
		const observer = new ResizeObserver(measure);
		observer.observe(bar);
		return () => observer.disconnect();
	}, []);

	if (toggles.length === 0) return null;

	const rows: MenuItem[] = toggles.map((toggle) => ({
		type: 'checkbox',
		id: toggle.id,
		label: toggle.label,
		checked: toggle.pressed,
		icon: createElement(toggle.icon),
		...(toggle.enabled ? {} : { disabled: true }),
	}));

	const openMenu = (keyboard: boolean) => {
		const box = moreRef.current?.getBoundingClientRect();
		setMenu({ position: { x: box?.left ?? 0, y: box?.bottom ?? 0 }, keyboard });
	};

	return (
		<div ref={groupRef} className={styles.group} role="group" aria-label={t('nav.panels.label')}>
			{collapsed ? (
				<button
					ref={moreRef}
					type="button"
					className={buttonStyles.button}
					aria-label={t('nav.panels.more')}
					title={t('nav.panels.more')}
					aria-haspopup="menu"
					aria-expanded={menu !== null}
					onClick={(event) => openMenu(event.detail === 0)}
				>
					<MoreIcon />
				</button>
			) : (
				toggles.map((toggle) => {
					const Icon = toggle.icon;
					return (
						<NavButton
							key={toggle.id}
							label={toggle.label}
							title={toggle.tooltip}
							pressed={toggle.pressed}
							icon={<Icon />}
							disabled={!toggle.enabled}
							onPress={() => run(toggle.id)}
						/>
					);
				})
			)}
			{collapsed && menu && (
				<ContextMenu
					items={rows}
					position={menu.position}
					ariaLabel={t('nav.panels.label')}
					returnFocusTo={moreRef.current}
					openedWithKeyboard={menu.keyboard}
					onSelect={(item) => {
						setMenu(null);
						run(item.id as (typeof toggles)[number]['id']);
					}}
					onClose={() => setMenu(null)}
				/>
			)}
		</div>
	);
}
