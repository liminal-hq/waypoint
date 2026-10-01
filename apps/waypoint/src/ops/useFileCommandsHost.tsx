// Builds the window's file commands over its queue, its file system client and a confirmation dialog
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { showNotice } from '../app/notices';
import type { ListingSession } from '../browse/useListingSession';
import { useVfsClient } from '../browse/VfsClientContext';
import { createClipboardService, type ClipboardService } from './clipboardService';
import { useConfirm } from './ConfirmHost';
import { createFileCommands, type ConfirmSpec, type FileCommands } from './fileCommands';
import { useOps } from './OpsContext';

export interface FileCommandsHost {
	/** `null` in a window with no queue. */
	commands: FileCommands | null;
	/** Asks a question in the chrome's dialog; the close guard uses it too. */
	confirm: (spec: ConfirmSpec) => Promise<boolean>;
	/** The dialog the commands ask their questions in; render it once. */
	dialog: ReactNode;
	/** The window's clipboard, which the views read to dim what a cut holds; `null` before it exists and without a queue. */
	clipboard: ClipboardService | null;
}

/**
 * `activeSession` reads the pane the keys act on at the moment a key is pressed. The commands are
 * made again only when the queue, the file system client or the window changes.
 */
export function useFileCommandsHost(
	activeSession: () => ListingSession | null,
	/** The pane a pair keeps beside a session, read when a command needs it. */
	otherPane?: (session: ListingSession) => ListingSession | null,
): FileCommandsHost {
	const ops = useOps();
	const vfs = useVfsClient();
	const { confirm, dialog } = useConfirm();
	const active = useRef(activeSession);
	active.current = activeSession;
	const other = useRef(otherPane);
	other.current = otherPane;
	const handle = ops?.handle;
	const windowLabel = ops?.windowLabel;
	const osClipboard = ops?.osClipboard ?? null;
	// Every window follows Rust's one clipboard and the system's, from the moment it has a queue.
	const [clipboard, setClipboard] = useState<ClipboardService | null>(null);
	useEffect(() => {
		if (!handle) return;
		const service = createClipboardService({
			client: handle.client,
			vfs,
			os: osClipboard,
			focusTarget: window,
		});
		setClipboard(service);
		return () => {
			service.dispose();
			setClipboard(null);
		};
	}, [handle, vfs, osClipboard]);
	const commands = useMemo(
		() =>
			handle && windowLabel
				? createFileCommands({
						ops: handle,
						vfs,
						windowLabel,
						activeSession: () => active.current(),
						confirm,
						say: (text) => void showNotice(text),
						clipboard,
						otherPane: (session) => other.current?.(session) ?? null,
					})
				: null,
		[handle, windowLabel, vfs, confirm, clipboard],
	);
	return { commands, confirm, dialog, clipboard };
}
