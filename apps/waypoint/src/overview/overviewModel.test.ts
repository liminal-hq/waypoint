// Verifies which volumes Overview counts, how it finds the one that holds Home, and the words of a bar
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { fakeVolume } from '../devices/fakeDevicesClient';
import {
	badgeOf,
	barOf,
	cardsOf,
	homeVolumeId,
	isCounted,
	spaceOf,
	statsOf,
	type SpaceState,
} from './overviewModel';
import { isOverviewLocation, OVERVIEW_LOCATION } from './overviewLocation';

const ROOT = fakeVolume('root', {
	label: 'System',
	kind: 'internal',
	isSystem: true,
	mountPoint: '/',
	fileSystem: 'ext4',
	total: 500_000_000_000,
	free: 200_000_000_000,
});
const DATA = fakeVolume('data', {
	label: 'Data',
	kind: 'internal',
	isSystem: true,
	mountPoint: '/home',
	fileSystem: 'btrfs',
	total: 1_000_000_000_000,
	free: 400_000_000_000,
});
const STICK = fakeVolume('stick', {
	label: 'Backup',
	mountPoint: '/run/media/test/Backup',
	fileSystem: 'exfat',
	total: 128_000_000_000,
	free: 100_000_000_000,
});
const NAS = fakeVolume('nas', {
	label: 'NAS',
	kind: 'network',
	isSystem: false,
	mountPoint: '/run/user/1000/gvfs/smb',
	fileSystem: 'cifs',
	total: null,
	free: null,
});
const LOOP = fakeVolume('snap', {
	label: 'core22',
	kind: 'loop',
	mountPoint: '/snap/core22',
	fileSystem: 'squashfs',
	total: 100_000_000,
	free: 0,
});

describe('overviewLocation', () => {
	it('recognises overview:/ in any spelling and nothing else', () => {
		expect(isOverviewLocation(OVERVIEW_LOCATION)).toBe(true);
		for (const uri of ['overview:', 'OVERVIEW:/', 'overview:///']) {
			expect(isOverviewLocation({ uri }), uri).toBe(true);
		}
		for (const uri of ['overview:/x', 'file:///overview:', 'trash:/']) {
			expect(isOverviewLocation({ uri }), uri).toBe(false);
		}
		expect(isOverviewLocation(undefined)).toBe(false);
	});
});

describe('the badge and the space of a volume', () => {
	it('says System for internal and system volumes and Removable for what is plugged in', () => {
		expect(badgeOf(ROOT)).toBe('system');
		expect(badgeOf(STICK)).toBe('removable');
		expect(badgeOf(NAS)).toBe('network');
		expect(badgeOf(fakeVolume('d', { kind: 'optical' }))).toBe('optical');
		expect(badgeOf(fakeVolume('e', { kind: 'encrypted', isSystem: false }))).toBe('removable');
	});

	it('tells each state of the space apart', () => {
		expect(spaceOf(STICK)).toMatchObject({ kind: 'measured', used: 28_000_000_000, percent: 22 });
		expect(spaceOf(NAS)).toEqual({ kind: 'unmeasured' });
		expect(spaceOf(fakeVolume('x', { total: null, free: null }))).toEqual({ kind: 'unavailable' });
		expect(spaceOf(fakeVolume('l', { locked: true, mountPoint: null }))).toEqual({
			kind: 'locked',
		});
		expect(spaceOf(fakeVolume('u', { mountPoint: null, free: null }))).toEqual({
			kind: 'notMounted',
		});
		// More free than the size, which a share can report, never makes a negative bar.
		expect(spaceOf(fakeVolume('o', { total: 100, free: 150 }))).toMatchObject({
			used: 0,
			free: 100,
			percent: 0,
		});
	});

	it('counts only local, mounted volumes that reported both numbers', () => {
		expect(isCounted(ROOT)).toBe(true);
		expect(isCounted(NAS)).toBe(false);
		expect(isCounted(LOOP)).toBe(false);
		expect(isCounted(fakeVolume('u', { mountPoint: null }))).toBe(false);
		expect(isCounted(fakeVolume('n', { free: null }))).toBe(false);
	});
});

describe('the volume that holds Home', () => {
	const all = [ROOT, DATA, STICK, NAS];

	it('is the one with the longest mount point that contains it', () => {
		expect(homeVolumeId(all, '/home/test')).toBe('data');
		expect(homeVolumeId([ROOT, STICK], '/home/test')).toBe('root');
		expect(homeVolumeId(all, '/homework')).toBe('root');
		expect(homeVolumeId(all, '/run/media/test/Backup/x')).toBe('stick');
	});

	it('is nobody when Home is unknown, and never a network volume or a disk image', () => {
		expect(homeVolumeId(all, null)).toBeNull();
		expect(homeVolumeId([NAS, LOOP], '/snap/core22/x')).toBeNull();
	});

	it('compares Windows drive paths without regard to case or separators', () => {
		const c = fakeVolume('c', { mountPoint: 'C:\\', kind: 'internal', isSystem: true });
		const d = fakeVolume('d', { mountPoint: 'D:\\' });
		expect(homeVolumeId([c, d], 'c:\\Users\\Test')).toBe('c');
		expect(homeVolumeId([c, d], 'D:/Users/Test')).toBe('d');
	});
});

describe('the headline stats', () => {
	const cards = cardsOf([ROOT, DATA, STICK, NAS, LOOP], '/home/test');

	it('leaves disk images out of the cards and out of every count', () => {
		expect(cards.map((card) => card.volume.id)).toEqual(['root', 'data', 'stick', 'nas']);
		const stats = statsOf(cards, { status: 'notMeasured' });
		expect(stats.volumeCount).toBe(4);
		expect(stats.fileSystems).toEqual(['ext4', 'btrfs', 'exfat', 'cifs']);
	});

	it('adds up capacity and free space over the local volumes only', () => {
		const stats = statsOf(cards, { status: 'notMeasured' });
		expect(stats.counted).toBe(3);
		expect(stats.capacity).toBe(1_628_000_000_000);
		expect(stats.free).toBe(700_000_000_000);
	});

	it('has no capacity when nothing reported a size', () => {
		const stats = statsOf(cardsOf([NAS], null), { status: 'notMeasured' });
		expect(stats.capacity).toBeNull();
		expect(stats.free).toBeNull();
	});

	it('carries the home folder as not measured, measuring, or a size and its share of its volume', () => {
		expect(statsOf(cards, { status: 'notMeasured' }).home).toEqual({ status: 'notMeasured' });
		expect(statsOf(cards, { status: 'measuring' }).home).toEqual({ status: 'measuring' });
		const done = statsOf(cards, { status: 'done', bytes: 120_000_000_000, asOfMs: null }).home;
		// Home is on Data, which has 600 GB in use.
		expect(done).toEqual({
			status: 'done',
			bytes: 120_000_000_000,
			sharePercent: 20,
			volume: 'Data',
		});
	});
});

describe('a bar', () => {
	const measured = (volume = STICK): Extract<SpaceState, { kind: 'measured' }> => {
		const space = spaceOf(volume);
		if (space.kind !== 'measured') throw new Error('not measured');
		return space;
	};

	it('has an exact Used part and a Free part, and a text equivalent that says the numbers', () => {
		const bar = barOf('Backup', measured(), null);
		expect(bar.segments.map((segment) => segment.key)).toEqual(['used', 'free']);
		expect(bar.segments[0]).toMatchObject({ bytes: 28_000_000_000, percent: 22 });
		expect(bar.segments[1]).toMatchObject({ bytes: 100_000_000_000, percent: 78 });
		expect(bar.text).toBe('Backup: 28 GB used and 100 GB free of 128 GB (22% used)');
		expect(bar.almostFull).toBe(false);
	});

	it('splits Used into Your files and Everything else, never past Used', () => {
		const bar = barOf('Data', measured(DATA), 120_000_000_000);
		expect(bar.segments.map((segment) => segment.key)).toEqual(['files', 'other', 'free']);
		expect(bar.segments[0]!.bytes + bar.segments[1]!.bytes).toBe(600_000_000_000);
		expect(bar.text).toContain('120 GB of your files and 480 GB of everything else');
		const over = barOf('Data', measured(DATA), 9_000_000_000_000);
		expect(over.segments[0]!.bytes).toBe(600_000_000_000);
		expect(over.segments[1]!.bytes).toBe(0);
	});

	it('says a nearly full volume is nearly full in the text as well as the pattern', () => {
		const tight = fakeVolume('t', { total: 1000, free: 50, label: 'Tight' });
		const bar = barOf('Tight', measured(tight), null);
		expect(bar.almostFull).toBe(true);
		expect(bar.text).toMatch(/Almost full\.$/);
	});
});
