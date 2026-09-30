// The toolbar row: back, forward and up with their history, and the path bar
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useCallback, useEffect, useState } from 'react';
import { BackIcon, ForwardIcon, UpIcon } from '../icons/AppIcons';
import { t } from '../i18n/messages';
import { NavButton } from './NavButton';
import styles from './NavigationBar.module.css';
import { PathBar } from './PathBar';
import { useNavigation, type Navigation } from './useNavigation';

/**
 * Keyboard paths for what the pointer does with the buttons (docs/accessibility.md item 9):
 * Alt+Left and Alt+Right for history, Alt+Up for the parent, Ctrl+L and Ctrl+Shift+G to type a
 * location. They live on the window because the file list, not the toolbar, usually has focus.
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
		window.addEventListener('keydown', onKeyDown);
		return () => window.removeEventListener('keydown', onKeyDown);
	}, [navigation, editPath]);
}

export function NavigationBar() {
	const navigation = useNavigation();
	const [editing, setEditing] = useState(false);
	const editPath = useCallback(() => setEditing(true), []);
	useNavigationShortcuts(navigation, editPath);

	const tab = navigation.tab;
	// Nearest first: the last entry of `back` is the folder just left, the last of `forward` the next.
	const behind = tab ? [...tab.back].reverse() : [];
	const ahead = tab ? [...tab.forward].reverse() : [];

	return (
		<div className={styles.bar} role="toolbar" aria-label={t('nav.toolbar.label')}>
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
				/>
			)}
		</div>
	);
}
