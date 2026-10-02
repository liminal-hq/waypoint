// Reads how the window's root element looks now, for a page that has to agree with it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useState } from 'react';
import { readSurfaceColours } from '../theme/transparencyDom';
import type { SurfaceColours } from '../theme/transparency';

/** Calls `listener` when an attribute of the root element changes (ThemeRoot writes the look there); returns the way to stop. */
function onRootChange(listener: () => void): () => void {
	if (typeof MutationObserver !== 'function') return () => undefined;
	const observer = new MutationObserver(listener);
	observer.observe(document.documentElement, { attributes: true });
	return () => observer.disconnect();
}

/** A `data-*` value of the root element, kept current; `null` when the attribute is absent. */
export function useRootData(key: string): string | null {
	const [value, setValue] = useState<string | null>(
		() => document.documentElement.dataset[key] ?? null,
	);
	useEffect(() => {
		const read = (): void => setValue(document.documentElement.dataset[key] ?? null);
		read();
		return onRootChange(read);
	}, [key]);
	return value;
}

const sameColours = (a: SurfaceColours, b: SurfaceColours): boolean =>
	(Object.keys(a) as (keyof SurfaceColours)[]).every((key) => a[key] === b[key]);

/** The colours of the theme in force, from the tokens: they change with the theme, so this follows the root. */
export function useSurfaceColours(): SurfaceColours {
	const [colours, setColours] = useState(() => readSurfaceColours(document.documentElement));
	useEffect(() => {
		const read = (): void => {
			const next = readSurfaceColours(document.documentElement);
			setColours((now) => (sameColours(now, next) ? now : next));
		};
		read();
		const stop = onRootChange(read);
		const query =
			typeof globalThis.matchMedia === 'function'
				? globalThis.matchMedia('(prefers-color-scheme: dark)')
				: null;
		query?.addEventListener?.('change', read);
		return () => {
			stop();
			query?.removeEventListener?.('change', read);
		};
	}, []);
	return colours;
}
