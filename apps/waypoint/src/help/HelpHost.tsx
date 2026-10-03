// Owns the Help dialog and its siblings for a Main window: opens them from F1, `?`, the menu and the palette
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useCallback, useEffect, useRef, useState } from 'react';
import { useCommandBridge, useCommands } from '../commands/commandBridge';
import type { AppInfoClient } from '../services/appInfoClient';
import { AboutDialog } from './AboutDialog';
import { HelpDialog } from './HelpDialog';
import { helpPageForKey } from './helpModel';
import type { HelpPage } from './helpPages';
import { KeyboardShortcutsDialog } from './KeyboardShortcutsDialog';
import { TourDialog } from './TourDialog';

interface HelpHostProps {
	/** Where About reads the version from. */
	appInfo?: AppInfoClient;
}

/**
 * Renders one of the four pages while one is open. Help leads to the others; each is a dialog of
 * its own, and moving between them hands the first one's return focus on to the next, so closing
 * the last puts focus back where it was before Help opened. F1 and `?` work from anywhere in the
 * window (`?` not while typing), switch between the pages when one is already open, and leave a
 * different dialog alone.
 */
export function HelpHost({ appInfo }: HelpHostProps) {
	const bridge = useCommandBridge();
	const { commands } = useCommands();
	const [page, setPage] = useState<HelpPage | null>(null);
	const current = useRef<HelpPage | null>(null);
	current.current = page;

	const open = useCallback((next: HelpPage) => {
		// A dialog of another kind has the person's attention; this one would stack over it.
		if (current.current === null && document.querySelector('dialog[open]')) return;
		setPage(next);
	}, []);
	const close = useCallback(() => setPage(null), []);

	useEffect(() => {
		bridge.patchActions({ openHelp: open });
	}, [bridge, open]);

	useEffect(() => {
		const onKeyDown = (event: KeyboardEvent) => {
			if (event.defaultPrevented) return;
			const next = helpPageForKey(event);
			if (next === null) return;
			event.preventDefault();
			open(next);
		};
		window.addEventListener('keydown', onKeyDown);
		return () => window.removeEventListener('keydown', onKeyDown);
	}, [open]);

	switch (page) {
		case 'help':
			return <HelpDialog key="help" views={commands} onClose={close} onOpen={open} />;
		case 'shortcuts':
			return <KeyboardShortcutsDialog key="shortcuts" views={commands} onClose={close} />;
		case 'tour':
			return <TourDialog key="tour" onClose={close} />;
		case 'about':
			return <AboutDialog key="about" client={appInfo} onClose={close} />;
		default:
			return null;
	}
}
