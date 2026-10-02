// The toolbar row: back, forward and up with their history, and the path bar
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useCallback, useEffect, useState, type ReactNode } from 'react';
import { BackIcon, ForwardIcon, UpIcon } from '../icons/AppIcons';
import { t } from '../i18n/messages';
import { NavButton } from './NavButton';
import styles from './NavigationBar.module.css';
import { requestPaneFocus } from '../tabs/paneFocus';
import { PathBar } from './PathBar';
import { useTabActions } from '../tabs/tabActions';
import { useWindowActions } from '../tabs/windowActions';
import { useNavigation, type Navigation } from './useNavigation';

/**
 * Keyboard paths for what the pointer does with the buttons (docs/accessibility.md item 9):
 * Alt+Left and Alt+Right for history, Alt+Up for the parent, Ctrl+L and Ctrl+Shift+G to type a
 * location, and the mouse's back and forward buttons. They live on the window because the file
 * list, not the toolbar, usually has focus.
 */
function useNavigationShortcuts(navigation: Navigation, editPath: () => void): void {
	useEffect(() => {
		const onKeyDown = (event: KeyboardEvent) => {
			if (event.defaultPrevented || event.isComposing) return;
			const key = event.key.toLowerCase();
			if (event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey) {
				const action =
					key === 'arrowleft'
						? navigation.back
						: key === 'arrowright'
							? navigation.forward
							: key === 'arrowup'
								? navigation.up
								: null;
				if (action) {
					event.preventDefault();
					action();
				}
				return;
			}
			const modifier = event.ctrlKey || event.metaKey;
			if (modifier && !event.altKey && (key === 'l' || (event.shiftKey && key === 'g'))) {
				event.preventDefault();
				editPath();
			}
		};
		// The mouse's side buttons (docs/interactions.md: "Back/forward buttons, anywhere"). They report
		// as buttons 3 and 4. The history step runs on release; the press and the click the browser
		// sends with it are cancelled too, so the webview never does its own history navigation.
		const sideButton = (event: MouseEvent) =>
			event.button === 3 ? navigation.back : event.button === 4 ? navigation.forward : null;
		const onMouseUp = (event: MouseEvent) => {
			const action = sideButton(event);
			if (!action) return;
			event.preventDefault();
			action();
		};
		const cancel = (event: MouseEvent) => {
			if (sideButton(event)) event.preventDefault();
		};
		window.addEventListener('keydown', onKeyDown);
		window.addEventListener('mouseup', onMouseUp, true);
		window.addEventListener('mousedown', cancel, true);
		window.addEventListener('auxclick', cancel, true);
		return () => {
			window.removeEventListener('keydown', onKeyDown);
			window.removeEventListener('mouseup', onMouseUp, true);
			window.removeEventListener('mousedown', cancel, true);
			window.removeEventListener('auxclick', cancel, true);
		};
	}, [navigation, editPath]);
}

interface NavigationBarProps {
	/** Controls that come before back and forward, such as the sidebar toggle. */
	leading?: ReactNode;
}

export function NavigationBar({ leading }: NavigationBarProps) {
	const navigation = useNavigation();
	const { openInBackground } = useTabActions();
	const { openInNewWindow } = useWindowActions();
	const [editing, setEditing] = useState(false);
	const editPath = useCallback(() => setEditing(true), []);
	useNavigationShortcuts(navigation, editPath);

	const tab = navigation.tab;
	// Nearest first: the last entry of `back` is the folder just left, the last of `forward` the next.
	const behind = tab ? [...tab.back].reverse() : [];
	const ahead = tab ? [...tab.forward].reverse() : [];

	return (
		<div className={styles.bar} role="toolbar" aria-label={t('nav.toolbar.label')}>
			{leading}
			<NavButton
				label={t('nav.back')}
				icon={<BackIcon />}
				disabled={!navigation.canGoBack}
				onPress={navigation.back}
				history={behind}
				menuLabel={t('nav.history.back')}
				onPick={(index) => navigation.backBy(index + 1)}
			/>
			<NavButton
				label={t('nav.forward')}
				icon={<ForwardIcon />}
				disabled={!navigation.canGoForward}
				onPress={navigation.forward}
				history={ahead}
				menuLabel={t('nav.history.forward')}
				onPick={(index) => navigation.forwardBy(index + 1)}
			/>
			<NavButton
				label={t('nav.up')}
				icon={<UpIcon />}
				disabled={!navigation.canGoUp}
				onPress={navigation.up}
			/>
			{tab && (
				<PathBar
					location={tab.location}
					editing={editing}
					onEditingChange={setEditing}
					onNavigate={navigation.goTo}
					onOpenInNewTab={(location, inNewWindow) =>
						inNewWindow ? openInNewWindow(location) : openInBackground(location)
					}
					// The list takes focus, so the arrow keys work straight after Enter.
					onCommitted={() => requestPaneFocus(tab.id)}
				/>
			)}
		</div>
	);
}
