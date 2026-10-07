// Where a menu icon is drawn to be read back as pixels: one hidden element, rendered into from the window's own tree when it has a host there
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ReactNode } from 'react';
import { flushSync } from 'react-dom';
import { createRoot, type Root } from 'react-dom/client';
import {
	createIconSettleTracker,
	IconSettleContext,
	type IconSettleTracker,
} from '../icons/iconSettle';
import styles from './MenuIconStage.module.css';

export interface IconStage {
	/** The hidden element in the document the icon is drawn in, so it takes the styles it has in a menu. */
	readonly element: HTMLElement;
	/** What the icons in the stage that are still arriving say, so the picture is taken once they are there. */
	readonly settle: IconSettleTracker;
	/** Draws `node` in the stage, and has done by the time it returns. */
	render(node: ReactNode): void;
	/** Takes the stage out of the document; the next `render` puts it back. */
	dispose(): void;
}

export interface HostedIconStage extends IconStage {
	/**
	 * Makes `host` (a state setter of `MenuIconStageHost`, in the window's tree) draw what is rendered, so the
	 * icons find the providers the window's own icons do. Returns the function that detaches it.
	 */
	attach(host: (node: ReactNode) => void): () => void;
}

/**
 * A stage for icons. Without a host it renders in a root of its own, which has no providers: enough for
 * an icon that is plain markup. With one, in the tree, an icon that reads a context (a location's
 * server) or a store through a hook draws as it does in the window.
 */
export function createIconStage(): HostedIconStage {
	const element = document.createElement('div');
	element.className = styles.stage ?? '';
	element.setAttribute('aria-hidden', 'true');
	const settle = createIconSettleTracker();
	let own: Root | null = null;
	let host: ((node: ReactNode) => void) | null = null;

	const mount = () => {
		if (!element.isConnected) document.body.append(element);
	};
	const unmountOwn = () => {
		const root = own;
		own = null;
		root?.unmount();
	};

	return {
		element,
		settle,
		render(node) {
			mount();
			if (host) {
				const draw = host;
				flushSync(() => draw(node));
				return;
			}
			own ??= createRoot(element);
			const root = own;
			flushSync(() =>
				root.render(<IconSettleContext.Provider value={settle}>{node}</IconSettleContext.Provider>),
			);
		},
		attach(next) {
			unmountOwn();
			host = next;
			mount();
			return () => {
				if (host === next) host = null;
			};
		},
		dispose() {
			unmountOwn();
			element.remove();
		},
	};
}
