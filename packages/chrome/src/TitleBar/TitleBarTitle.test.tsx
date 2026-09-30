// Tests that the title text is a drag region carrying its children
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { TitleBarTitle } from './TitleBarTitle';

describe('TitleBarTitle', () => {
	it('renders its children in a drag region', () => {
		render(<TitleBarTitle className="extra">Documents</TitleBarTitle>);
		const title = screen.getByText('Documents');
		expect(title.tagName).toBe('SPAN');
		expect(title).toHaveAttribute('data-tauri-drag-region');
		expect(title).toHaveClass('extra');
	});
});
