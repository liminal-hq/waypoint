// Draws the native menu's icon stage from inside the window's tree, so the icons in it have the window's providers
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { IconSettleContext } from '../icons/iconSettle';
import type { HostedIconStage } from './iconStage';
import { useNativeMenuService } from './NativeMenuContext';

/** Draws what `stage` is asked to render into its hidden element, from where this is in the tree. */
export function IconStagePortal({ stage }: { stage: HostedIconStage }) {
	const [node, setNode] = useState<ReactNode>(null);
	useEffect(() => {
		const detach = stage.attach(setNode);
		return () => {
			detach();
			setNode(null);
		};
	}, [stage]);
	return createPortal(
		<IconSettleContext.Provider value={stage.settle}>{node}</IconSettleContext.Provider>,
		stage.element,
	);
}

/**
 * Mount once, below the providers the window's icons read (the file system and the connections). It
 * renders nothing where it is: what the rasteriser asks the stage to draw is portalled into the
 * stage's hidden element, so a location's icon there finds the same contexts it does in a menu.
 */
export function MenuIconStageHost() {
	const stage = useNativeMenuService()?.stage;
	return stage ? <IconStagePortal stage={stage} /> : null;
}
