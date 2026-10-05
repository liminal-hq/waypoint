// Verifies a notice's further buttons and what runs once it has gone
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { dismissNotice, showNotice } from './notices';
import { NoticeToast } from './NoticeToast';

afterEach(() => {
	act(() => dismissNotice());
	cleanup();
});

describe('a notice with more than one action', () => {
	it('shows each as a button that runs it and closes the notice, then says it closed', async () => {
		const resume = vi.fn();
		const discard = vi.fn();
		const closed = vi.fn();
		render(<NoticeToast />);
		act(() => {
			showNotice(
				'A transfer stopped',
				{ label: 'Resume', run: resume },
				{ more: [{ label: 'Discard…', run: discard }], onClose: closed },
			);
		});
		await userEvent.click(screen.getByRole('button', { name: 'Discard…' }));
		expect(discard).toHaveBeenCalledOnce();
		expect(resume).not.toHaveBeenCalled();
		expect(closed).toHaveBeenCalledOnce();
		expect(screen.queryByText('A transfer stopped')).toBeNull();
	});

	it('does not say it closed when another notice replaces it', () => {
		const closed = vi.fn();
		act(() => {
			showNotice('first', undefined, { onClose: closed });
			showNotice('second');
		});
		expect(closed).not.toHaveBeenCalled();
	});
});
