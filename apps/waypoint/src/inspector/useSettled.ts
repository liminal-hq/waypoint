// A value that has held still for a while, for work that is not worth starting on every keypress
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useState } from 'react';

/**
 * `true` once `key` has been the same for `delayMs`, and `false` from the moment it changes until
 * then. The cheap parts of the Inspector follow a selection at once; what reads files waits for it
 * to settle, so holding an arrow key through a folder starts nothing.
 */
export function useSettled(key: string, delayMs: number): boolean {
	const [settled, setSettled] = useState<string | null>(null);
	useEffect(() => {
		const timer = setTimeout(() => setSettled(key), delayMs);
		return () => clearTimeout(timer);
	}, [key, delayMs]);
	return settled === key;
}
