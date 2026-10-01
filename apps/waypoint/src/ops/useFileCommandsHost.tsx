// Builds the window's file commands over its queue, its file system client and a confirmation dialog
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useMemo, useRef, type ReactNode } from 'react';
import { showNotice } from '../app/notices';
import type { ListingSession } from '../browse/useListingSession';
import { useVfsClient } from '../browse/VfsClientContext';
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
}

/**
 * `activeSession` reads the pane the keys act on at the moment a key is pressed. The commands are
 * made again only when the queue, the file system client or the window changes.
 */
export function useFileCommandsHost(activeSession: () => ListingSession | null): FileCommandsHost {
	const ops = useOps();
	const vfs = useVfsClient();
	const { confirm, dialog } = useConfirm();
	const active = useRef(activeSession);
	active.current = activeSession;
	const handle = ops?.handle;
	const windowLabel = ops?.windowLabel;
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
					})
				: null,
		[handle, windowLabel, vfs, confirm],
	);
	return { commands, confirm, dialog };
}
