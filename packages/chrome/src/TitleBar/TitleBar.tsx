// Unified Liminal title bar with slots, window controls and window menu
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useCallback, useEffect, useState, type MouseEvent, type ReactNode } from 'react';
import { CloseIcon, MaximiseIcon, MinimiseIcon, PinIcon, RestoreIcon } from '../icons/icons';
import { defaultChromeLabels, type ChromeLabels } from '../labels';
import type { MenuPosition } from '../ContextMenu/types';
import { WindowMenu } from '../WindowMenu/WindowMenu';
import { isInteractiveTarget } from './interactive';
import styles from './TitleBar.module.css';
import { useFocused } from './useFocused';
import { useMaximised } from './useMaximised';
import type { ControlsSide, ControlsStyle, WindowControls } from './windowControls';

export interface TitleBarProps {
	windowControls: WindowControls;
	/** Left-hand slot: app mark and menu button. */
	start?: ReactNode;
	/** Flexible middle slot: title and app content. */
	center?: ReactNode;
	/** App actions, next to the window buttons. */
	end?: ReactNode;
	controlsStyle?: ControlsStyle;
	controlsSide?: ControlsSide;
	/** Shows the Always on Top toggle (and its window menu entry). */
	showAlwaysOnTop?: boolean;
	/** Lets the desktop show through; opacity comes from `--wp-title-bar-opacity`. */
	transparent?: boolean;
	labels?: Partial<ChromeLabels>;
	className?: string;
}

export function TitleBar({
	windowControls,
	start,
	center,
	end,
	controlsStyle = 'gnome',
	controlsSide = 'end',
	showAlwaysOnTop = false,
	transparent = false,
	labels: labelOverrides,
	className,
}: TitleBarProps) {
	const labels = { ...defaultChromeLabels, ...labelOverrides };
	const maximised = useMaximised(windowControls);
	const focused = useFocused(windowControls);
	const [alwaysOnTop, setAlwaysOnTop] = useState(false);
	const [menuPosition, setMenuPosition] = useState<MenuPosition | null>(null);

	useEffect(() => {
		let active = true;
		if (windowControls.isAlwaysOnTop) {
			Promise.resolve(windowControls.isAlwaysOnTop())
				.then((value) => {
					if (active) setAlwaysOnTop(value);
				})
				.catch(() => {});
		}
		return () => {
			active = false;
		};
	}, [windowControls]);

	const changeAlwaysOnTop = useCallback(
		(value: boolean) => {
			setAlwaysOnTop(value);
			void windowControls.setAlwaysOnTop(value);
		},
		[windowControls],
	);

	// Only space the bar itself owns opens the menu; portalled menu events also bubble here.
	const isEmptySpace = (event: MouseEvent<HTMLElement>) =>
		event.currentTarget.contains(event.target as Node) && !isInteractiveTarget(event.target);

	const onContextMenu = (event: MouseEvent<HTMLElement>) => {
		if (!isEmptySpace(event)) return;
		event.preventDefault();
		setMenuPosition({ x: event.clientX, y: event.clientY });
	};

	const onDoubleClick = (event: MouseEvent<HTMLElement>) => {
		if (windowControls.handlesDoubleClickNatively || !isEmptySpace(event)) return;
		void windowControls.toggleMaximize();
	};

	const controls = (
		<WindowButtons
			windowControls={windowControls}
			controlsStyle={controlsStyle}
			controlsSide={controlsSide}
			maximised={maximised}
			showAlwaysOnTop={showAlwaysOnTop}
			alwaysOnTop={alwaysOnTop}
			onAlwaysOnTopChange={changeAlwaysOnTop}
			labels={labels}
		/>
	);

	const classes = [styles.titleBar, maximised ? styles.maximised : '', className ?? '']
		.filter(Boolean)
		.join(' ');

	return (
		<>
			<div
				className={classes}
				data-tauri-drag-region=""
				data-maximised={maximised}
				data-focused={focused}
				data-transparent={transparent || undefined}
				data-controls-style={controlsStyle}
				data-controls-side={controlsSide}
				onContextMenu={onContextMenu}
				onDoubleClick={onDoubleClick}
			>
				{controlsSide === 'start' ? controls : null}
				{start ? (
					<div className={styles.start} data-tauri-drag-region="">
						{start}
					</div>
				) : null}
				<div className={styles.center} data-tauri-drag-region="">
					{center}
				</div>
				{end ? (
					<div className={styles.end} data-tauri-drag-region="">
						{end}
					</div>
				) : null}
				{controlsSide === 'end' ? controls : null}
			</div>
			{menuPosition ? (
				<WindowMenu
					controls={windowControls}
					position={menuPosition}
					isMaximised={maximised}
					alwaysOnTop={alwaysOnTop}
					showAlwaysOnTop={showAlwaysOnTop}
					onAlwaysOnTopChange={changeAlwaysOnTop}
					onClose={() => setMenuPosition(null)}
					labels={labels}
				/>
			) : null}
		</>
	);
}

interface WindowButtonsProps {
	windowControls: WindowControls;
	controlsStyle: ControlsStyle;
	controlsSide: ControlsSide;
	maximised: boolean;
	showAlwaysOnTop: boolean;
	alwaysOnTop: boolean;
	onAlwaysOnTopChange: (value: boolean) => void;
	labels: ChromeLabels;
}

function WindowButtons({
	windowControls,
	controlsStyle,
	controlsSide,
	maximised,
	showAlwaysOnTop,
	alwaysOnTop,
	onAlwaysOnTopChange,
	labels,
}: WindowButtonsProps) {
	const minimise = (
		<button
			key="minimise"
			type="button"
			className={`${styles.button} ${styles.minimise}`}
			aria-label={labels.minimise}
			title={labels.minimise}
			onClick={() => void windowControls.minimize()}
		>
			<MinimiseIcon />
		</button>
	);
	const maximiseLabel = maximised ? labels.restore : labels.maximise;
	const maximise = (
		<button
			key="maximise"
			type="button"
			className={`${styles.button} ${styles.maximise}`}
			aria-label={maximiseLabel}
			title={maximiseLabel}
			onClick={() => void windowControls.toggleMaximize()}
		>
			{maximised ? <RestoreIcon /> : <MaximiseIcon />}
		</button>
	);
	const close = (
		<button
			key="close"
			type="button"
			className={`${styles.button} ${styles.close}`}
			aria-label={labels.close}
			title={labels.close}
			onClick={() => void windowControls.close()}
		>
			<CloseIcon />
		</button>
	);
	// Close sits on the outer edge of the window, whichever side the buttons are on.
	const ordered =
		controlsSide === 'start' ? [close, minimise, maximise] : [minimise, maximise, close];

	return (
		<div
			className={styles.controls}
			role="group"
			aria-label={labels.windowControls}
			data-wp-controls=""
			data-controls-style={controlsStyle}
		>
			{showAlwaysOnTop ? (
				<button
					type="button"
					className={`${styles.button} ${styles.pin}`}
					aria-label={labels.alwaysOnTop}
					title={labels.alwaysOnTop}
					aria-pressed={alwaysOnTop}
					onClick={() => onAlwaysOnTopChange(!alwaysOnTop)}
				>
					<PinIcon />
				</button>
			) : null}
			{ordered}
		</div>
	);
}
