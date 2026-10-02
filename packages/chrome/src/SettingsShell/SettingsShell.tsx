// Two-pane settings layout: a side nav of sections beside the active page, collapsing to a select
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useId, useRef, useState, type KeyboardEvent, type ReactNode } from 'react';
import '../tokens.css';
import { defaultSettingsLabels, SettingsLabelsContext, type SettingsLabels } from './labels';
import styles from './SettingsShell.module.css';
import type { SettingsSectionDef } from './types';

/** Width in pixels below which the side nav becomes a select above the page. Matches the 640 px design breakpoint. */
export const SETTINGS_COLLAPSE_WIDTH = 640;

export interface SettingsShellProps {
	sections: SettingsSectionDef[];
	/** Id of the section shown. Controlled: the app owns it. */
	activeId: string;
	onSelect: (id: string) => void;
	/** Shell width in pixels under which the nav collapses to a select. */
	collapseBelow?: number;
	/** Reserved above the page heading for a search box. Nothing is built into the shell yet. */
	toolbar?: ReactNode;
	labels?: Partial<SettingsLabels>;
}

export function SettingsShell({
	sections,
	activeId,
	onSelect,
	collapseBelow = SETTINGS_COLLAPSE_WIDTH,
	toolbar,
	labels,
}: SettingsShellProps) {
	const mergedLabels = { ...defaultSettingsLabels, ...labels };
	const shellRef = useRef<HTMLDivElement>(null);
	const navRef = useRef<HTMLElement>(null);
	const headingId = useId();
	const panelId = useId();
	const [collapsed, setCollapsed] = useState(false);

	useEffect(() => {
		const shell = shellRef.current;
		if (!shell || typeof ResizeObserver === 'undefined') return;
		const measure = (width: number) => setCollapsed(width > 0 && width < collapseBelow);
		measure(shell.getBoundingClientRect().width);
		const observer = new ResizeObserver((entries) => {
			const entry = entries[entries.length - 1];
			if (entry) measure(entry.contentRect.width);
		});
		observer.observe(shell);
		return () => observer.disconnect();
	}, [collapseBelow]);

	const active: SettingsSectionDef | undefined =
		sections.find((section) => section.id === activeId) ?? sections[0];
	const activeIndex = active ? sections.indexOf(active) : 0;

	const onNavKeyDown = (event: KeyboardEvent<HTMLElement>) => {
		const buttons = [
			...(navRef.current?.querySelectorAll<HTMLElement>('button[data-section]') ?? []),
		];
		if (buttons.length === 0) return;
		const current = buttons.indexOf(document.activeElement as HTMLElement);
		const from = current < 0 ? activeIndex : current;
		let next = -1;
		if (event.key === 'ArrowDown') next = (from + 1) % buttons.length;
		else if (event.key === 'ArrowUp') next = (from - 1 + buttons.length) % buttons.length;
		else if (event.key === 'Home') next = 0;
		else if (event.key === 'End') next = buttons.length - 1;
		if (next < 0) return;
		event.preventDefault();
		buttons[next]?.focus();
	};

	if (!active) return null;
	return (
		<SettingsLabelsContext.Provider value={mergedLabels}>
			<div
				ref={shellRef}
				className={`${styles.shell} ${collapsed ? styles.collapsed : ''}`}
				data-collapsed={collapsed ? '' : undefined}
			>
				{collapsed ? (
					<div className={styles.select}>
						<select
							aria-label={mergedLabels.navigation}
							className={styles.selectField}
							value={active.id}
							onChange={(event) => onSelect(event.target.value)}
						>
							{sections.map((section) => (
								<option key={section.id} value={section.id}>
									{section.label}
								</option>
							))}
						</select>
					</div>
				) : (
					<nav
						ref={navRef}
						className={styles.nav}
						aria-label={mergedLabels.navigation}
						onKeyDown={onNavKeyDown}
					>
						<ul className={styles.navList}>
							{sections.map((section) => {
								const isActive = section.id === active.id;
								return (
									<li key={section.id}>
										<button
											type="button"
											data-section={section.id}
											aria-current={isActive ? 'page' : undefined}
											aria-controls={panelId}
											tabIndex={isActive ? 0 : -1}
											className={`${styles.navItem} ${isActive ? styles.navItemActive : ''}`}
											onClick={() => onSelect(section.id)}
										>
											{section.icon ? (
												<span className={styles.navIcon} aria-hidden="true">
													{section.icon}
												</span>
											) : null}
											<span className={styles.navLabel}>{section.label}</span>
										</button>
									</li>
								);
							})}
						</ul>
					</nav>
				)}
				<main id={panelId} className={styles.content} aria-labelledby={headingId}>
					{toolbar ? <div className={styles.toolbar}>{toolbar}</div> : null}
					<h2 id={headingId} className={styles.heading}>
						{active.label}
					</h2>
					{active.render()}
				</main>
			</div>
		</SettingsLabelsContext.Provider>
	);
}
