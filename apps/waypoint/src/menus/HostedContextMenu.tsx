// A context menu that opens as the system's own menu when the setting is on, and as the page's own otherwise
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ContextMenu, type ContextMenuProps } from '@liminal-hq/waypoint-chrome/ContextMenu';
import { useEffect, useRef, useState, type ReactNode } from 'react';
import type { NativeMenuIcon } from './nativeMenuClient';
import { collectIcons, toNativeMenu } from './nativeMenu';
import { useNativeContextMenusSetting } from './nativeMenuSetting';
import { useNativeMenuService } from './NativeMenuContext';

/** How long a menu whose items are still arriving is held before the page shows its own instead. */
export const NATIVE_SETTLE_MS = 400;

export interface HostedContextMenuProps extends ContextMenuProps {
	/**
	 * The items are still being read (the applications of Open With). A native menu cannot change
	 * once it is up, so it waits for this to clear, for a moment at most; the page's own menu is
	 * shown at once and fills in as the items arrive.
	 */
	settling?: boolean;
}

type Mode = 'deciding' | 'native' | 'page';

/** How long the icons may take to draw before the page shows its own menu: a menu is never kept waiting on them. */
export const NATIVE_PREPARE_MS = 1500;

const timedOut = Symbol('timed out');

/**
 * The seam between a menu's model and how it is drawn. It takes a `ContextMenu`'s props. With
 * Settings → Experimental → Native context menus on, in a window that has the service, it converts
 * the items and asks the system to show them at the pointer; the chosen item runs the same
 * `onSelect` the page's menu would, and a dismissed menu runs `onClose`. When the items cannot be
 * shown natively, or the system cannot show them, it renders the page's `ContextMenu` instead, so a
 * menu is never lost. A menu opened again at another place (`position`) is shown again from there.
 */
export function HostedContextMenu({ settling = false, ...props }: HostedContextMenuProps) {
	const enabled = useNativeContextMenusSetting();
	const service = useNativeMenuService();
	const usable = enabled && service !== null && !service.state.failed;
	const [mode, setMode] = useState<Mode>(usable ? 'deciding' : 'page');
	const [trigger] = useState<Element | null>(() => props.returnFocusTo ?? document.activeElement);
	const latest = useRef(props);
	latest.current = props;
	const run = useRef(0);
	const startedAt = useRef<string | null>(null);
	const shown = useRef(false);
	const alive = useRef(true);
	const { x, y } = props.position;

	// The menu is held at most this long while its items settle.
	useEffect(() => {
		if (mode !== 'deciding' || !settling) return;
		const timer = window.setTimeout(() => setMode('page'), NATIVE_SETTLE_MS);
		return () => window.clearTimeout(timer);
	}, [mode, settling]);

	useEffect(() => {
		if (mode === 'page' || settling || !service) return;
		const place = `${x},${y}`;
		if (startedAt.current === place) return;
		startedAt.current = place;
		const mine = ++run.current;
		const current = () => alive.current && run.current === mine;
		void (async () => {
			let timer: number | undefined;
			try {
				const prepare = (async () => {
					const { items } = latest.current;
					const pictures = new Map<ReactNode, NativeMenuIcon | null>();
					for (const icon of collectIcons(items)) pictures.set(icon, await service.rasterise(icon));
					return toNativeMenu(items, (icon) => pictures.get(icon));
				})();
				const expiry = new Promise<typeof timedOut>((resolve) => {
					timer = window.setTimeout(() => resolve(timedOut), NATIVE_PREPARE_MS);
				});
				const converted = await Promise.race([prepare, expiry]);
				window.clearTimeout(timer);
				if (!current()) return;
				if (converted === timedOut || !converted.ok) {
					console.debug(
						`native menu: shown by the page, because ${
							converted === timedOut ? 'its icons took too long' : converted.reason
						}`,
					);
					setMode('page');
					return;
				}
				shown.current = true;
				setMode('native');
				const chosen = await service.client.show(converted.items, { x, y });
				if (!current()) return;
				const { onSelect, onClose } = latest.current;
				const item = chosen === null ? undefined : converted.selectable.get(chosen);
				if (item) onSelect(item);
				onClose();
			} catch (error) {
				window.clearTimeout(timer);
				if (!current()) return;
				console.warn('native menu: the system could not show it, so the page does', error);
				service.state.failed = true;
				shown.current = false;
				setMode('page');
			}
		})();
	}, [mode, settling, service, x, y]);

	// A page menu puts the focus back where it was; a native one does the same when it is gone.
	useEffect(() => {
		alive.current = true;
		return () => {
			alive.current = false;
			if (
				shown.current &&
				trigger instanceof HTMLElement &&
				trigger.isConnected &&
				trigger !== document.body
			) {
				trigger.focus();
			}
		};
	}, [trigger]);

	return mode === 'page' ? <ContextMenu {...props} /> : null;
}
