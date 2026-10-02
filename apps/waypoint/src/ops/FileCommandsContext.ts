// Gives the file views and the menus the window's file commands, or `null` where nothing can be written
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useContext } from 'react';
import type { FileCommands } from './fileCommands';

const FileCommandsContext = createContext<FileCommands | null>(null);

export const FileCommandsProvider = FileCommandsContext.Provider;

/** The commands for this window; `null` in a window with no queue (the views then offer no rename). */
export function useFileCommands(): FileCommands | null {
	return useContext(FileCommandsContext);
}
