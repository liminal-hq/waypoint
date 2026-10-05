// Verifies the S3 service table: the endpoint each preset makes, what an endpoint reads back as, and that the page's table agrees with the provider's
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { S3_PRESETS, s3Endpoint, s3PresetOf } from './s3Presets';

describe('the endpoint a preset makes', () => {
	it('fills each template with what the person typed', () => {
		expect(s3Endpoint('b2', ' us-west-004 ')).toBe('https://s3.us-west-004.backblazeb2.com');
		expect(s3Endpoint('r2', 'abc123')).toBe('https://abc123.r2.cloudflarestorage.com');
		expect(s3Endpoint('wasabi', 'eu-central-1')).toBe('https://s3.eu-central-1.wasabisys.com');
		expect(s3Endpoint('spaces', 'ams3')).toBe('https://ams3.digitaloceanspaces.com');
	});

	it('is no endpoint for Amazon S3, and takes a typed server with https unless it says http', () => {
		expect(s3Endpoint('aws', '')).toBeNull();
		expect(s3Endpoint('minio', 'minio.lan:9000')).toBe('https://minio.lan:9000');
		expect(s3Endpoint('minio', 'http://minio.lan:9000')).toBe('http://minio.lan:9000');
		expect(s3Endpoint('custom', 'https://s3.example.com')).toBe('https://s3.example.com');
	});

	it('is undefined while a preset that needs an input has none', () => {
		for (const id of ['b2', 'r2', 'wasabi', 'minio', 'spaces', 'custom']) {
			expect(s3Endpoint(id, '  ')).toBeUndefined();
		}
	});
});

describe('what an endpoint reads back as', () => {
	it('finds the preset and its input in a templated endpoint', () => {
		expect(s3PresetOf('https://s3.us-west-004.backblazeb2.com')).toEqual({
			preset: 'b2',
			value: 'us-west-004',
		});
		expect(s3PresetOf('https://abc123.r2.cloudflarestorage.com')).toEqual({
			preset: 'r2',
			value: 'abc123',
		});
		expect(s3PresetOf(null)).toEqual({ preset: 'aws', value: '' });
	});

	it('keeps a chosen preset without a template, and calls an unknown endpoint custom', () => {
		expect(s3PresetOf('https://minio.lan:9000', 'minio')).toEqual({
			preset: 'minio',
			value: 'minio.lan:9000',
		});
		expect(s3PresetOf('https://s3.example.com')).toEqual({
			preset: 'custom',
			value: 'https://s3.example.com',
		});
	});
});

describe('the table agrees with the provider’s', () => {
	// Vitest runs from `apps/waypoint`.
	const rust = readFileSync(
		join(process.cwd(), '..', '..', 'crates', 'waypoint-provider-s3', 'src', 'presets.rs'),
		'utf8',
	);

	it('has the same services, templates and path style', () => {
		for (const preset of S3_PRESETS.filter((p) => p.id !== 'custom')) {
			const block = rust.slice(rust.indexOf(`id: "${preset.id}"`));
			expect(block, preset.id).toContain(`id: "${preset.id}"`);
			const body = block.slice(0, block.indexOf('};'));
			if (preset.template) expect(body).toContain(preset.template.replace('{value}', '{value}'));
			expect(body, preset.id).toContain(`path_style: ${preset.pathStyle}`);
		}
	});
});
