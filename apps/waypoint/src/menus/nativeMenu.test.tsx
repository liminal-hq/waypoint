// Verifies how a context menu's items become a native menu description, and when they cannot
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import { describe, expect, it } from 'vitest';
import {
	collectIcons,
	MAX_NATIVE_DEPTH,
	MAX_NATIVE_ICON_BYTES,
	MAX_NATIVE_ITEMS,
	MAX_NATIVE_TEXT,
	toNativeMenu,
} from './nativeMenu';
import type { NativeMenuIcon } from './nativeMenuClient';

type IconFor = Parameters<typeof toNativeMenu>[1];

const noIcons: IconFor = () => undefined;

function picture(bytes = 4): NativeMenuIcon {
	return { width: 1, height: bytes / 4, rgba: new Array<number>(bytes).fill(255) };
}

function converted(items: MenuItem[], iconFor: IconFor = noIcons) {
	const result = toNativeMenu(items, iconFor);
	if (!result.ok) throw new Error(`not converted: ${result.reason}`);
	return result;
}

describe('toNativeMenu', () => {
	it('turns every kind of item into its native counterpart, keeping the page’s ids', () => {
		const result = converted([
			{ type: 'action', id: 'open', label: 'Open', shortcut: 'Enter' },
			{ type: 'separator' },
			{ type: 'checkbox', id: 'hidden', label: 'Show hidden', checked: true },
			{
				type: 'submenu',
				id: 'sort',
				label: 'Sort by',
				items: [{ type: 'checkbox', id: 'sort:name', label: 'Name', checked: false }],
			},
		]);
		expect(result.items).toEqual([
			{ kind: 'action', id: 'open', label: 'Open', enabled: true, shortcut: 'Enter' },
			{ kind: 'separator' },
			{ kind: 'checkbox', id: 'hidden', label: 'Show hidden', checked: true, enabled: true },
			{
				kind: 'submenu',
				label: 'Sort by',
				enabled: true,
				items: [
					{ kind: 'checkbox', id: 'sort:name', label: 'Name', checked: false, enabled: true },
				],
			},
		]);
		expect([...result.selectable.keys()]).toEqual(['open', 'hidden', 'sort:name']);
	});

	it('hands back the very items the in-page menu would pass to onSelect', () => {
		const open = { type: 'action', id: 'open', label: 'Open' } as const;
		const nested = { type: 'checkbox', id: 'name', label: 'Name', checked: true } as const;
		const result = converted([
			open,
			{ type: 'submenu', id: 'sort', label: 'Sort', items: [nested] },
		]);
		expect(result.selectable.get('open')).toBe(open);
		expect(result.selectable.get('name')).toBe(nested);
	});

	it('carries disabled through, and a danger action is an ordinary one', () => {
		const result = converted([
			{ type: 'action', id: 'a', label: 'A', disabled: true },
			{ type: 'action', id: 'b', label: 'Delete', danger: true, title: 'Gone for good' },
			{ type: 'submenu', id: 's', label: 'S', disabled: true, items: [] },
			{ type: 'checkbox', id: 'c', label: 'C', checked: false, disabled: true },
		]);
		expect(result.items.map((item) => ('enabled' in item ? item.enabled : null))).toEqual([
			false,
			true,
			false,
			false,
		]);
		expect(result.items[1]).not.toHaveProperty('danger');
		expect(result.items[1]).not.toHaveProperty('title');
	});

	it('shows a section heading as a disabled item that cannot be chosen', () => {
		const result = converted([
			{ type: 'section', label: 'Sort by' },
			{ type: 'action', id: 'a', label: 'A' },
		]);
		expect(result.items[0]).toMatchObject({ kind: 'action', label: 'Sort by', enabled: false });
		expect(result.selectable.has((result.items[0] as { id: string }).id)).toBe(false);
	});

	it('gives each section its own id, so two headings are not confused', () => {
		const result = converted([
			{ type: 'section', label: 'Sort by' },
			{ type: 'action', id: 'a', label: 'A' },
			{ type: 'section', label: 'Group by' },
		]);
		const ids = result.items.filter((item) => item.kind === 'action').map((item) => item.id);
		expect(new Set(ids).size).toBe(ids.length);
	});

	it('passes a shortcut on to actions and checkboxes, and has none to give a submenu', () => {
		const result = converted([
			{ type: 'action', id: 'a', label: 'A', shortcut: 'Ctrl+C' },
			{ type: 'checkbox', id: 'b', label: 'B', checked: false, shortcut: 'Ctrl+B' },
			{ type: 'action', id: 'c', label: 'C' },
		]);
		expect(result.items.map((item) => ('shortcut' in item ? item.shortcut : undefined))).toEqual([
			'Ctrl+C',
			'Ctrl+B',
			undefined,
		]);
	});

	it('draws an action’s icon from the lookup, and goes without one that has no picture', () => {
		const withPicture = <svg data-testid="a" />;
		const withoutPicture = <span />;
		const made = picture();
		const result = converted(
			[
				{ type: 'action', id: 'a', label: 'A', icon: withPicture },
				{ type: 'action', id: 'b', label: 'B', icon: withoutPicture },
				{ type: 'action', id: 'c', label: 'C' },
			],
			(icon) => (icon === withPicture ? made : null),
		);
		expect(result.items[0]).toMatchObject({ icon: made });
		expect(result.items[1]).not.toHaveProperty('icon');
		expect(result.items[2]).not.toHaveProperty('icon');
		expect(result.droppedIcons).toBe(1);
	});

	it('drops the icon of a checkbox and of a submenu, which neither can carry', () => {
		const result = converted(
			[
				{ type: 'checkbox', id: 'a', label: 'A', checked: true, icon: <svg /> },
				{
					type: 'submenu',
					id: 's',
					label: 'S',
					icon: <svg />,
					items: [{ type: 'action', id: 'b', label: 'B' }],
				},
			],
			() => picture(),
		);
		expect(result.items[0]).not.toHaveProperty('icon');
		expect(result.items[1]).not.toHaveProperty('icon');
		expect(result.droppedIcons).toBe(2);
	});

	it('keeps to the icon byte budget by dropping the icons past it', () => {
		const big = picture(MAX_NATIVE_ICON_BYTES / 2 + 4);
		const result = converted(
			[
				{ type: 'action', id: 'a', label: 'A', icon: <svg /> },
				{ type: 'action', id: 'b', label: 'B', icon: <svg /> },
				{ type: 'action', id: 'c', label: 'C', icon: <svg /> },
			],
			() => big,
		);
		expect(result.items.filter((item) => 'icon' in item)).toHaveLength(1);
		expect(result.droppedIcons).toBe(2);
	});

	it('clips a label that is too long, ending it with an ellipsis', () => {
		const result = converted([
			{ type: 'action', id: 'a', label: 'é'.repeat(MAX_NATIVE_TEXT + 40) },
		]);
		const label = (result.items[0] as { label: string }).label;
		expect(Array.from(label)).toHaveLength(MAX_NATIVE_TEXT);
		expect(label.endsWith('…')).toBe(true);
	});

	it('leaves a label that fits as it is', () => {
		const result = converted([{ type: 'action', id: 'a', label: 'Sort & Group' }]);
		expect((result.items[0] as { label: string }).label).toBe('Sort & Group');
	});

	describe('when the page keeps its own menu', () => {
		const action: MenuItem = { type: 'action', id: 'a', label: 'A' };

		it('has nothing in it to choose', () => {
			for (const items of [
				[],
				[{ type: 'separator' }],
				[{ type: 'section', label: 'Only a heading' }],
				[{ type: 'submenu', id: 's', label: 'S', items: [] }],
			] as MenuItem[][]) {
				expect(toNativeMenu(items, noIcons).ok).toBe(false);
			}
		});

		it('has more items than the command takes, counting those in submenus', () => {
			const many = (count: number): MenuItem[] =>
				Array.from({ length: count }, (_, n) => ({
					type: 'action',
					id: `item-${n}`,
					label: 'Item',
				}));
			expect(toNativeMenu(many(MAX_NATIVE_ITEMS), noIcons).ok).toBe(true);
			expect(toNativeMenu(many(MAX_NATIVE_ITEMS + 1), noIcons).ok).toBe(false);
			expect(
				toNativeMenu(
					[{ type: 'submenu', id: 's', label: 'S', items: many(MAX_NATIVE_ITEMS) }],
					noIcons,
				).ok,
			).toBe(false);
		});

		it('nests submenus deeper than the command takes', () => {
			const nest = (levels: number): MenuItem[] => {
				let items: MenuItem[] = [action];
				for (let level = 0; level < levels; level += 1) {
					items = [{ type: 'submenu', id: `s${level}`, label: 'S', items }];
				}
				return items;
			};
			expect(toNativeMenu(nest(MAX_NATIVE_DEPTH), noIcons).ok).toBe(true);
			const refused = toNativeMenu(nest(MAX_NATIVE_DEPTH + 1), noIcons);
			expect(refused).toEqual({ ok: false, reason: expect.stringContaining('nested') });
		});

		it('has an id longer than the command takes', () => {
			const result = toNativeMenu(
				[{ type: 'action', id: 'x'.repeat(MAX_NATIVE_TEXT + 1), label: 'A' }],
				noIcons,
			);
			expect(result).toEqual({ ok: false, reason: expect.stringContaining('id') });
		});
	});
});

describe('collectIcons', () => {
	it('lists the icons of actions, in order, through submenus, and skips the items that cannot carry one', () => {
		const a = <svg data-testid="a" />;
		const b = <svg data-testid="b" />;
		const c = <svg data-testid="c" />;
		expect(
			collectIcons([
				{ type: 'action', id: 'a', label: 'A', icon: a },
				{ type: 'section', label: 'Heading' },
				{ type: 'separator' },
				{ type: 'action', id: 'none', label: 'No icon' },
				{
					type: 'submenu',
					id: 's',
					label: 'S',
					icon: <svg data-testid="submenu" />,
					items: [{ type: 'action', id: 'b', label: 'B', icon: b }],
				},
				{ type: 'checkbox', id: 'c', label: 'C', checked: false, icon: c },
			]),
		).toEqual([a, b]);
	});
});
