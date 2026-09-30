// Types describing context menu content
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ReactNode } from 'react';

export interface MenuPosition {
	x: number;
	y: number;
}

interface LabelledMenuItem {
	id: string;
	label: string;
	/** Rendered in the 16px icon slot. The slot is always reserved so labels align. */
	icon?: ReactNode;
	shortcut?: string;
	disabled?: boolean;
}

export interface ActionMenuItem extends LabelledMenuItem {
	type: 'action';
	/** Styles the row as a destructive action. */
	danger?: boolean;
}

export interface CheckboxMenuItem extends LabelledMenuItem {
	type: 'checkbox';
	checked: boolean;
}

export interface SubmenuMenuItem extends Omit<LabelledMenuItem, 'shortcut'> {
	type: 'submenu';
	items: MenuItem[];
}

export interface SeparatorMenuItem {
	type: 'separator';
	id?: string;
}

export interface SectionLabelMenuItem {
	type: 'section';
	id?: string;
	label: string;
}

/** Items that can be chosen and report back through `onSelect`. */
export type SelectableMenuItem = ActionMenuItem | CheckboxMenuItem;

export type MenuItem =
	ActionMenuItem | CheckboxMenuItem | SubmenuMenuItem | SeparatorMenuItem | SectionLabelMenuItem;
