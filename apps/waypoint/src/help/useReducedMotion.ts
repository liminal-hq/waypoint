// Whether the system asks for less motion, kept current while the window is open
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useState } from 'react';

const QUERY = '(prefers-reduced-motion: reduce)';

function query(): MediaQueryList | null {
	try {
		return globalThis.matchMedia?.(QUERY) ?? null;
	} catch {
		return null;
	}
}

/** True while the system's "reduce motion" setting is on; follows the setting changing. */
export function useReducedMotion(): boolean {
	const [reduced, setReduced] = useState(() => query()?.matches ?? false);
	useEffect(() => {
		const list = query();
		if (!list) return;
		const update = () => setReduced(list.matches);
		update();
		list.addEventListener?.('change', update);
		return () => list.removeEventListener?.('change', update);
	}, []);
	return reduced;
}
