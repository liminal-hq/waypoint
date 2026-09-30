// Tests for the pure window menu model builder
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { buildWindowMenuModel } from './windowMenuModel';

const ids = (items: ReturnType<typeof buildWindowMenuModel>) =>
	items.filter((i) => i.type !== 'separator').map((i) => i.id);

describe('buildWindowMenuModel', () => {
	it('offers Maximise when the window is not maximised', () => {
		const items = buildWindowMenuModel({ isMaximised: false, alwaysOnTop: false });
		expect(ids(items)).toEqual(['maximise', 'minimise', 'move', 'always-on-top', 'close']);
		expect(items[0]).toMatchObject({ label: 'Maximise' });
	});

	it('offers Restore when the window is maximised', () => {
		const items = buildWindowMenuModel({ isMaximised: true, alwaysOnTop: false });
		expect(ids(items)[0]).toBe('restore');
		expect(items[0]).toMatchObject({ label: 'Restore' });
	});

	it('puts Move, Always on Top and Close each in their own section', () => {
		const items = buildWindowMenuModel({ isMaximised: false, alwaysOnTop: false });
		expect(items.map((i) => (i.type === 'separator' ? '|' : i.id)).join(' ')).toBe(
			'maximise minimise | move | always-on-top | close',
		);
	});

	it('reflects the Always on Top state on the checkbox', () => {
		const on = buildWindowMenuModel({ isMaximised: false, alwaysOnTop: true });
		const off = buildWindowMenuModel({ isMaximised: false, alwaysOnTop: false });
		expect(on.find((i) => i.id === 'always-on-top')).toMatchObject({
			type: 'checkbox',
			checked: true,
		});
		expect(off.find((i) => i.id === 'always-on-top')).toMatchObject({ checked: false });
	});

	it('omits Always on Top and Move when unavailable', () => {
		const items = buildWindowMenuModel({
			isMaximised: false,
			alwaysOnTop: false,
			showAlwaysOnTop: false,
			canMove: false,
		});
		expect(ids(items)).toEqual(['maximise', 'minimise', 'close']);
	});

	it('uses supplied labels', () => {
		const items = buildWindowMenuModel({
			isMaximised: false,
			alwaysOnTop: false,
			labels: {
				restore: 'R',
				maximise: 'Agrandir',
				minimise: 'M',
				move: 'D',
				alwaysOnTop: 'T',
				systemWindowMenu: 'S',
				close: 'Fermer',
			},
		});
		expect(items[0]).toMatchObject({ label: 'Agrandir' });
		expect(items.at(-1)).toMatchObject({ label: 'Fermer' });
	});

	it('adds More options in its own section before Close when the host can show it', () => {
		const items = buildWindowMenuModel({
			isMaximised: false,
			alwaysOnTop: false,
			canShowSystemMenu: true,
		});
		expect(items.map((i) => (i.type === 'separator' ? '|' : i.id)).join(' ')).toBe(
			'maximise minimise | move | always-on-top | system-menu | close',
		);
	});

	it('omits More options by default', () => {
		const items = buildWindowMenuModel({ isMaximised: false, alwaysOnTop: false });
		expect(items.some((i) => i.id === 'system-menu')).toBe(false);
	});
});
