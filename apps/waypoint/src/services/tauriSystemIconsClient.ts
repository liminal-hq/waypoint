// The real SystemIconsClient: the file and folder icons of the mime-apps plugin, and the OS look from the system-appearance plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import {
	featureMessage,
	getStatus,
	hasFeature,
	refreshTypeIcons,
	typeIconUrl,
	type Feature,
} from '@liminal-hq/plugin-mime-apps';
import { getAppearance, onAppearanceChanged } from '@liminal-hq/plugin-system-appearance';
import type { SystemIconAvailability, SystemIconsClient, SystemLook } from './systemIconsClient';

function look(preferences: { iconTheme: string | null; colourScheme: string }): SystemLook {
	return { theme: preferences.iconTheme, scheme: preferences.colourScheme };
}

/** A `SystemIconsClient` over the plugins. Outside Tauri `status` and `look` reject and the caller keeps the Waypoint icons. */
export function createTauriSystemIconsClient(): SystemIconsClient {
	return {
		async status() {
			const status = await getStatus();
			const of = (feature: Feature): SystemIconAvailability => ({
				available: hasFeature(status, feature),
				reason: featureMessage(status, feature) ?? null,
			});
			return { typeIcons: of('typeIcons'), folderIcons: of('folderIcons') };
		},
		async look() {
			return look(await getAppearance());
		},
		onLookChanged(listener) {
			let unlisten: (() => void) | undefined;
			let stopped = false;
			void (async () => onAppearanceChanged((preferences) => listener(look(preferences))))().then(
				(stop) => {
					if (stopped) stop();
					else unlisten = stop;
				},
				() => undefined,
			);
			return () => {
				stopped = true;
				unlisten?.();
				unlisten = undefined;
			};
		},
		refresh: () => refreshTypeIcons(),
		url: (target, options) => typeIconUrl(target, options),
		probe(url, done) {
			const image = new Image();
			let cancelled = false;
			image.onload = () => {
				if (!cancelled) done(true);
			};
			image.onerror = () => {
				if (!cancelled) done(false);
			};
			image.src = url;
			return () => {
				cancelled = true;
				image.onload = null;
				image.onerror = null;
			};
		},
	};
}
