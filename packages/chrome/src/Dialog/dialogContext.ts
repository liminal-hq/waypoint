// Context that lets dialog buttons ask their dialog to close
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useContext } from 'react';

/** Why a dialog asks to be closed. The app decides what each reason means. */
export type DialogCloseReason = 'escape' | 'cancel' | 'backdrop' | 'action';

export interface DialogContextValue {
	requestClose: (reason: DialogCloseReason) => void;
}

export const DialogContext = createContext<DialogContextValue | null>(null);

export function useDialogContext(): DialogContextValue | null {
	return useContext(DialogContext);
}
