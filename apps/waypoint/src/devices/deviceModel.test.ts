// Tests for the Devices section's pure model: usage text, which rows show, which actions, and failure sentences
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { actionsFor, failureText, statusText, usageOf, visibleVolumes } from './deviceModel';
import { fakeStatus, fakeVolume } from './fakeDevicesClient';

describe('usageOf', () => {
	it('writes the free and total space and the share used', () => {
		const usage = usageOf(fakeVolume('a', { total: 500_000_000_000, free: 125_000_000_000 }));
		expect(usage?.percent).toBe(75);
		expect(usage?.text).toMatch(/125.*free of.*500/);
		expect(usage?.almostFull).toBe(false);
		expect(usage?.warning).toBeNull();
	});

	it('says "Almost full" in words from 90 percent', () => {
		const usage = usageOf(fakeVolume('a', { total: 1000, free: 50 }));
		expect(usage?.almostFull).toBe(true);
		expect(usage?.text).toContain('Almost full');
		expect(usage?.warning).toBe('Almost full');
		expect(usage?.summary).not.toContain('Almost full');
	});

	it('has nothing to show without a size or a measurement', () => {
		expect(usageOf(fakeVolume('a', { free: null }))).toBeNull();
		expect(usageOf(fakeVolume('a', { total: null }))).toBeNull();
		expect(usageOf(fakeVolume('a', { total: 0 }))).toBeNull();
	});
});

describe('statusText', () => {
	it('names the state when there is no space to show', () => {
		expect(statusText(fakeVolume('a', { locked: true, mountPoint: null }))).toBe('Locked');
		expect(statusText(fakeVolume('a', { mountPoint: null, free: null, total: null }))).toBe(
			'Not mounted',
		);
		expect(statusText(fakeVolume('a', { free: null }))).toBe('Free space unavailable');
	});
});

describe('visibleVolumes', () => {
	it('leaves out the system partitions that are not mounted', () => {
		const shown = visibleVolumes([
			fakeVolume('root', { isSystem: true }),
			fakeVolume('swap', { isSystem: true, mountPoint: null, canMount: true }),
			fakeVolume('stick', { mountPoint: null, canMount: true }),
			fakeVolume('vault', { mountPoint: null, locked: true }),
			fakeVolume('dead', { mountPoint: null }),
		]);
		expect(shown.map((volume) => volume.id)).toEqual(['root', 'stick', 'vault']);
	});
});

describe('actionsFor', () => {
	it('offers what the volume allows', () => {
		expect(actionsFor(fakeVolume('a'), fakeStatus())).toEqual(['unmount', 'eject']);
		expect(actionsFor(fakeVolume('a', { mountPoint: null, canMount: true }), fakeStatus())).toEqual(
			['mount', 'eject'],
		);
		expect(actionsFor(fakeVolume('a', { locked: true, mountPoint: null }), fakeStatus())).toEqual([
			'unlock',
		]);
	});

	it('hides what the plugin says does not work here', () => {
		expect(actionsFor(fakeVolume('a'), fakeStatus(['eject']))).toEqual(['unmount']);
		expect(actionsFor(fakeVolume('a'), fakeStatus(['unmount', 'eject']))).toEqual([]);
		expect(actionsFor(fakeVolume('a', { locked: true }), fakeStatus(['unlock']))).toEqual([]);
		expect(actionsFor(fakeVolume('a'), null)).toEqual([]);
	});
});

describe('failureText', () => {
	it('says what is using a busy volume', () => {
		expect(failureText('unmount', 'Backup', { kind: 'busy', by: 'bash' })).toBe(
			'Could not unmount Backup because bash is using it.',
		);
		expect(failureText('eject', 'Backup', { kind: 'busy', by: null })).toBe(
			'Could not eject Backup because something is still using it.',
		);
	});

	it('gives each other reason in words', () => {
		expect(failureText('mount', 'A', { kind: 'notAuthorised' })).toContain('permission');
		expect(failureText('mount', 'A', { kind: 'notFound' })).toContain('no longer there');
		expect(failureText('mount', 'A', { kind: 'io', message: 'bad superblock' })).toContain(
			'bad superblock',
		);
		expect(failureText('mount', 'A', new Error('x'))).toBe('Could not mount A.');
	});
});
