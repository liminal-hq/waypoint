// Hook tracking whether the host window has focus
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useState } from 'react';
import type { WindowControls } from './windowControls';

/**
 * Reads the initial focus state, then follows changes. A host that cannot report focus is
 * treated as always focused. Pass a stable `controls` object.
 */
export function useFocused(controls: Pick<WindowControls, 'isFocused' | 'onFocusChange'>): boolean {
	const [focused, setFocused] = useState(true);

	useEffect(() => {
		let active = true;
		if (controls.isFocused) {
			Promise.resolve(controls.isFocused())
				.then((value) => {
					if (active) setFocused(value);
				})
				.catch(() => {});
		}
		const unsubscribe = controls.onFocusChange?.((value) => {
			if (active) setFocused(value);
		});
		return () => {
			active = false;
			unsubscribe?.();
		};
	}, [controls]);

	return focused;
}
