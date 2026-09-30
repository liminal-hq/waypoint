// Hook tracking whether the host window is maximised
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useState } from 'react';
import type { WindowControls } from './windowControls';

/** Reads the initial maximised state, then follows changes. Pass a stable `controls` object. */
export function useMaximised(
	controls: Pick<WindowControls, 'isMaximized' | 'onMaximizedChange'>,
): boolean {
	const [maximised, setMaximised] = useState(false);

	useEffect(() => {
		let active = true;
		Promise.resolve(controls.isMaximized())
			.then((value) => {
				if (active) setMaximised(value);
			})
			.catch(() => {});
		const unsubscribe = controls.onMaximizedChange((value) => {
			if (active) setMaximised(value);
		});
		return () => {
			active = false;
			unsubscribe();
		};
	}, [controls]);

	return maximised;
}
