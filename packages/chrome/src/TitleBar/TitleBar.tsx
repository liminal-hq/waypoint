// Unified Liminal title bar with slots, window controls and window menu
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useCallback, useEffect, useState, type MouseEvent, type ReactNode } from 'react';
import { CloseIcon, MaximiseIcon, MinimiseIcon, PinIcon, RestoreIcon } from '../icons/icons';
import { defaultChromeLabels, type ChromeLabels } from '../labels';
import type { MenuPosition } from '../ContextMenu/types';
import {
	useWindowControls,
	useWindowFocused,
	useWindowMaximised,
} from '../WindowChromeProvider/WindowChromeProvider';
import { WindowMenu } from '../WindowMenu/WindowMenu';
import {
	DEFAULT_BUTTON_LAYOUT,
	DEFAULT_TITLEBAR_ACTIONS,
	type ButtonLayout,
	type ChromeButton,
	type TitlebarAction,
	type TitlebarActions,
} from './buttonLayout';
import { isInteractiveTarget } from './interactive';
import '../tokens.css';
import styles from './TitleBar.module.css';
import type { ControlsStyle, WindowControls } from './windowControls';

export type TitleAlign = 'center' | 'start';

export interface TitleBarProps {
	/** Left-hand slot: app mark and menu button. Sits after any start-side window buttons. */
	start?: ReactNode;
	/** Flexible middle slot: title and app content. */
	center?: ReactNode;
	/** App actions, next to the end-side window buttons. */
	end?: ReactNode;
	controlsStyle?: ControlsStyle;
	/** Which window buttons sit on each side. Tokens the chrome does not support are skipped. */
	buttonLayout?: ButtonLayout;
	/** What double, middle and right click on empty bar space do. */
	titlebarActions?: TitlebarActions;
	/** `center` keeps the centre slot exactly centred; `start` left-aligns it after the start group. */
	titleAlign?: TitleAlign;
	/** Shows the Always on Top toggle (and its window menu entry). */
	showAlwaysOnTop?: boolean;
	/** Lets the desktop show through; opacity comes from `--wp-title-bar-opacity`. */
	transparent?: boolean;
	labels?: Partial<ChromeLabels>;
	className?: string;
}

const WINDOW_BUTTONS: ChromeButton[] = ['minimise', 'maximise', 'close'];

/** Places the Always on Top pin: where the layout says, or before the end side's first window button. */
function resolveSides(layout: ButtonLayout, showAlwaysOnTop: boolean) {
	const start = [...layout.start];
	const end = [...layout.end];
	if (showAlwaysOnTop && !start.includes('keepAbove') && !end.includes('keepAbove')) {
		const at = end.findIndex((token) => WINDOW_BUTTONS.includes(token));
		end.splice(at === -1 ? end.length : at, 0, 'keepAbove');
	}
	return { start, end };
}

export function TitleBar({
	start,
	center,
	end,
	controlsStyle = 'gnome',
	buttonLayout = DEFAULT_BUTTON_LAYOUT,
	titlebarActions = DEFAULT_TITLEBAR_ACTIONS,
	titleAlign = 'center',
	showAlwaysOnTop = false,
	transparent = false,
	labels: labelOverrides,
	className,
}: TitleBarProps) {
	const labels = { ...defaultChromeLabels, ...labelOverrides };
	const windowControls = useWindowControls();
	const maximised = useWindowMaximised();
	const focused = useWindowFocused();
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

	// Only space the bar itself owns runs actions; portalled menu events also bubble here.
	const isEmptySpace = (event: MouseEvent<HTMLElement>) =>
		event.currentTarget.contains(event.target as Node) && !isInteractiveTarget(event.target);

	const run = (action: TitlebarAction, event: MouseEvent<HTMLElement>, native = false) => {
		switch (action) {
			case 'toggleMaximise':
				if (!native) void windowControls.toggleMaximize();
				break;
			case 'minimise':
				void windowControls.minimize();
				break;
			case 'menu':
				setMenuPosition({ x: event.clientX, y: event.clientY });
				break;
			default:
				break;
		}
	};

	const onContextMenu = (event: MouseEvent<HTMLElement>) => {
		if (titlebarActions.rightClick !== 'menu' || !isEmptySpace(event)) return;
		event.preventDefault();
		run('menu', event);
	};

	const onDoubleClick = (event: MouseEvent<HTMLElement>) => {
		if (!isEmptySpace(event)) return;
		run(titlebarActions.doubleClick, event, windowControls.handlesDoubleClickNatively);
	};

	const onAuxClick = (event: MouseEvent<HTMLElement>) => {
		if (event.button !== 1 || !isEmptySpace(event)) return;
		run(titlebarActions.middleClick, event);
	};

	const sides = resolveSides(buttonLayout, showAlwaysOnTop);
	const renderControls = (side: 'start' | 'end') => (
		<WindowButtons
			side={side}
			tokens={sides[side]}
			windowControls={windowControls}
			controlsStyle={controlsStyle}
			maximised={maximised}
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
				data-title-align={titleAlign}
				onContextMenu={onContextMenu}
				onDoubleClick={onDoubleClick}
				onAuxClick={onAuxClick}
			>
				<div className={styles.startGroup} data-tauri-drag-region="" data-group="start">
					{renderControls('start')}
					{start ? (
						<div className={styles.start} data-tauri-drag-region="">
							{start}
						</div>
					) : null}
				</div>
				<div className={styles.center} data-tauri-drag-region="" data-group="centre">
					{center}
				</div>
				<div className={styles.endGroup} data-tauri-drag-region="" data-group="end">
					{end ? (
						<div className={styles.end} data-tauri-drag-region="">
							{end}
						</div>
					) : null}
					{renderControls('end')}
				</div>
			</div>
			{menuPosition ? (
				<WindowMenu
					position={menuPosition}
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
	side: 'start' | 'end';
	tokens: ChromeButton[];
	windowControls: WindowControls;
	controlsStyle: ControlsStyle;
	maximised: boolean;
	alwaysOnTop: boolean;
	onAlwaysOnTopChange: (value: boolean) => void;
	labels: ChromeLabels;
}

function WindowButtons({
	side,
	tokens,
	windowControls,
	controlsStyle,
	maximised,
	alwaysOnTop,
	onAlwaysOnTopChange,
	labels,
}: WindowButtonsProps) {
	const buttons = tokens.flatMap((token, index) => {
		const key = `${token}-${index}`;
		switch (token) {
			case 'minimise':
				return [
					<button
						key={key}
						type="button"
						className={`${styles.button} ${styles.minimise}`}
						aria-label={labels.minimise}
						title={labels.minimise}
						onClick={() => void windowControls.minimize()}
					>
						<MinimiseIcon />
					</button>,
				];
			case 'maximise': {
				const label = maximised ? labels.restore : labels.maximise;
				return [
					<button
						key={key}
						type="button"
						className={`${styles.button} ${styles.maximise}`}
						aria-label={label}
						title={label}
						onClick={() => void windowControls.toggleMaximize()}
					>
						{maximised ? <RestoreIcon /> : <MaximiseIcon />}
					</button>,
				];
			}
			case 'close':
				return [
					<button
						key={key}
						type="button"
						className={`${styles.button} ${styles.close}`}
						aria-label={labels.close}
						title={labels.close}
						onClick={() => void windowControls.close()}
					>
						<CloseIcon />
					</button>,
				];
			case 'keepAbove':
				return [
					<button
						key={key}
						type="button"
						className={`${styles.button} ${styles.pin}`}
						aria-label={labels.alwaysOnTop}
						title={labels.alwaysOnTop}
						aria-pressed={alwaysOnTop}
						onClick={() => onAlwaysOnTopChange(!alwaysOnTop)}
					>
						<PinIcon />
					</button>,
				];
			default:
				// appMenu, windowMenu, keepBelow, shade, stick and help are not chrome buttons.
				return [];
		}
	});
	if (buttons.length === 0) return null;

	return (
		<div
			className={styles.controls}
			role="group"
			aria-label={labels.windowControls}
			data-wp-controls=""
			data-controls-style={controlsStyle}
			data-controls-side={side}
		>
			{buttons}
		</div>
	);
}
