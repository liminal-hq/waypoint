// An in-memory OpenWithClient for tests and the browser demo: scripted features and applications, and a log of what was started
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type {
	App,
	FeatureStatus,
	Handlers,
	MimeAppsError,
	PluginStatus,
} from '@liminal-hq/plugin-mime-apps';
import type { OpenWithClient } from './openWithClient';

/** The features a working Linux desktop reports. */
const LINUX_FEATURES = [
	'typeInfo',
	'handlers',
	'openWith',
	'openDefault',
	'setDefault',
	'appIcons',
];

export interface FakeOpenWithOptions {
	/** The features that work; the rest report as unavailable. A Linux desktop's by default. */
	features?: readonly string[];
	/** What `handlers` answers for any locations. */
	handlers?: Partial<Handlers>;
	/** What `choose` does: resolve, or reject with this error. */
	chooseError?: MimeAppsError;
	/** Rejects every start (`openWith`, `openDefault`) with this. */
	openError?: MimeAppsError;
}

/** A tiny application for a test. */
export function fakeApp(id: string, name = id): App {
	return { id, name, icon: null, execHint: null };
}

export interface FakeOpenWithClient extends OpenWithClient {
	/** Every call that changes something, in order. */
	calls: Array<['openWith', string[], string] | ['openDefault', string[]] | ['choose', string[]]>;
	/** The locations `handlers` was asked about, in order. */
	asked: string[][];
}

/** A client whose status and applications are the options', that records what it was asked to open. */
export function createFakeOpenWithClient(options: FakeOpenWithOptions = {}): FakeOpenWithClient {
	const available = new Set(options.features ?? LINUX_FEATURES);
	const features: FeatureStatus[] = [
		'typeInfo',
		'handlers',
		'openWith',
		'openDefault',
		'setDefault',
		'chooser',
		'appIcons',
	].map((name) => ({
		name,
		available: available.has(name),
		reason: available.has(name) ? null : 'not-implemented',
		message: null,
	}));
	const status: PluginStatus = {
		available: available.size > 0,
		reason: null,
		message: null,
		flavour: 'gio',
		features,
		associationFiles: [],
	};
	const calls: FakeOpenWithClient['calls'] = [];
	const asked: string[][] = [];
	return {
		calls,
		asked,
		getStatus: async () => status,
		handlers: async (uris) => {
			asked.push(uris);
			return {
				mime: 'text/plain',
				mixed: false,
				default: null,
				recommended: [],
				others: [],
				...options.handlers,
			};
		},
		openWith: async (uris, appId) => {
			if (options.openError) throw options.openError;
			calls.push(['openWith', uris, appId]);
		},
		openDefault: async (uris) => {
			if (options.openError) throw options.openError;
			calls.push(['openDefault', uris]);
		},
		choose: async (uris) => {
			if (options.chooseError) throw options.chooseError;
			calls.push(['choose', uris]);
		},
		iconUrl: (appId, size = 32) => `appicon://localhost/${encodeURIComponent(appId)}?size=${size}`,
	};
}
