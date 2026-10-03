// The Inspector's empty state says what is missing and what to do
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { EmptyState } from './EmptyState';

describe('EmptyState', () => {
	it('shows a heading and a hint', () => {
		render(<EmptyState />);
		expect(screen.getByText('Nothing selected')).toBeInTheDocument();
		expect(
			screen.getByText('Select a file to see its preview and properties.'),
		).toBeInTheDocument();
	});
});
