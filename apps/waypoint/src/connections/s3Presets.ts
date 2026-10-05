// The S3 services the Connect dialog offers: their endpoint shapes and what each needs, as the provider's table has them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MessageId } from '../i18n/messages';

/** What the one input under a preset asks for. */
export type S3Input = 'none' | 'region' | 'accountId' | 'host' | 'endpoint';

export interface S3Preset {
	/** Saved with the connection (`s3Preset`) so the form shows it again. */
	id: string;
	label: MessageId;
	/** The endpoint with `{value}` where the input goes; `null` when there is none (AWS) or the input is the whole endpoint. */
	template: string | null;
	input: S3Input;
	/** Whether the bucket goes in the path (`host/bucket/key`): safe wherever it is accepted, and needed where there is no wildcard DNS for buckets. */
	pathStyle: boolean;
	/** The host-name ending that recognises an endpoint typed by hand as this service. */
	hostSuffix: string | null;
}

/**
 * In the order the dialog lists them. It mirrors `crates/waypoint-provider-s3/src/presets.rs`, which
 * decides how a connection is signed and addressed (the provider reads the endpoint's host, not this
 * table); the page needs the shapes to fill the endpoint in, and a test keeps the two in step.
 */
export const S3_PRESETS: readonly S3Preset[] = [
	{
		id: 'aws',
		label: 'connect.s3.preset.aws',
		template: null,
		input: 'none',
		pathStyle: false,
		hostSuffix: '.amazonaws.com',
	},
	{
		id: 'b2',
		label: 'connect.s3.preset.b2',
		template: 'https://s3.{value}.backblazeb2.com',
		input: 'region',
		pathStyle: true,
		hostSuffix: '.backblazeb2.com',
	},
	{
		id: 'r2',
		label: 'connect.s3.preset.r2',
		template: 'https://{value}.r2.cloudflarestorage.com',
		input: 'accountId',
		pathStyle: true,
		hostSuffix: '.r2.cloudflarestorage.com',
	},
	{
		id: 'wasabi',
		label: 'connect.s3.preset.wasabi',
		template: 'https://s3.{value}.wasabisys.com',
		input: 'region',
		pathStyle: true,
		hostSuffix: '.wasabisys.com',
	},
	{
		id: 'minio',
		label: 'connect.s3.preset.minio',
		template: null,
		input: 'host',
		pathStyle: true,
		hostSuffix: null,
	},
	{
		id: 'spaces',
		label: 'connect.s3.preset.spaces',
		template: 'https://{value}.digitaloceanspaces.com',
		input: 'region',
		pathStyle: true,
		hostSuffix: '.digitaloceanspaces.com',
	},
	{
		id: 'custom',
		label: 'connect.s3.preset.custom',
		template: null,
		input: 'endpoint',
		pathStyle: true,
		hostSuffix: null,
	},
];

export function s3Preset(id: string): S3Preset {
	return S3_PRESETS.find((preset) => preset.id === id) ?? S3_PRESETS[S3_PRESETS.length - 1]!;
}

/** Whether `text` already says its scheme (`http://minio.lan:9000`). */
const hasScheme = (text: string): boolean => /^[a-z][a-z0-9+.-]*:\/\//i.test(text);

/**
 * The endpoint a preset and its input make: the template filled in, or what was typed (with
 * `https://` when it names no scheme: write `http://` for a server without TLS). `null` is AWS, and
 * for a preset that needs an input, `undefined` while it is blank.
 */
export function s3Endpoint(presetId: string, value: string): string | null | undefined {
	const preset = s3Preset(presetId);
	const input = value.trim();
	if (preset.input === 'none') return null;
	if (input === '') return undefined;
	if (preset.template) return preset.template.replace('{value}', input);
	return hasScheme(input) ? input : `https://${input}`;
}

/** The preset and input an endpoint reads back as: a template that matches, a host ending that is known, else a custom endpoint. */
export function s3PresetOf(
	endpoint: string | null,
	saved?: string | null,
): { preset: string; value: string } {
	if (endpoint === null) return { preset: 'aws', value: '' };
	const known = saved ? S3_PRESETS.find((preset) => preset.id === saved) : undefined;
	const candidates = known ? [known] : S3_PRESETS;
	for (const preset of candidates) {
		if (preset.template) {
			const [head, tail] = preset.template.split('{value}') as [string, string];
			if (
				endpoint.startsWith(head) &&
				endpoint.endsWith(tail) &&
				endpoint.length > head.length + tail.length
			) {
				return {
					preset: preset.id,
					value: endpoint.slice(head.length, endpoint.length - tail.length),
				};
			}
		}
	}
	// A preset the person chose that has no template keeps its own input (MinIO's host, a custom endpoint).
	if (known && !known.template && known.input !== 'none') {
		return { preset: known.id, value: endpoint.replace(/^https:\/\//, '') };
	}
	return { preset: 'custom', value: endpoint };
}
