// Verifies the application chooser's filtering, its sections and how an application is chosen
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { fakeApp } from './fakeOpenWithClient';
import type { ChooserRequest } from './openWithChooserStore';
import { chooserSections, OpenWithDialog } from './OpenWithDialog';

const request = (scope: ChooserRequest['scope']): ChooserRequest => ({
	uris: ['file:///a.png'],
	scope,
	handlers: {
		mime: 'image/png',
		mixed: false,
		default: fakeApp('viewer', 'Image Viewer'),
		recommended: [fakeApp('editor', 'Image Editor')],
		others: [fakeApp('text', 'Text Editor'), fakeApp('calc', 'Calculator')],
	},
});

describe('the sections', () => {
	it('list only the other applications for the menu’s Other Application…, and everything from the command', () => {
		expect(chooserSections(request('others'), '').map((s) => s.key)).toEqual(['others']);
		const all = chooserSections(request('all'), '');
		expect(all.map((s) => s.key)).toEqual(['recommended', 'others']);
		expect(all[0]!.apps.map((app) => app.name)).toEqual(['Image Viewer', 'Image Editor']);
	});

	it('filter by name, ignoring case, and drop a section with no match', () => {
		const found = chooserSections(request('all'), ' EDITOR ');
		expect(found.map((s) => s.apps.map((app) => app.name))).toEqual([
			['Image Editor'],
			['Text Editor'],
		]);
		expect(chooserSections(request('all'), 'zzz')).toEqual([]);
	});
});

describe('the dialog', () => {
	afterEach(cleanup);
	const open = (scope: ChooserRequest['scope'] = 'others') => {
		const onChoose = vi.fn();
		const onCancel = vi.fn();
		render(
			<OpenWithDialog
				request={request(scope)}
				iconUrl={(id) => `appicon://localhost/${id}`}
				onChoose={onChoose}
				onCancel={onCancel}
			/>,
		);
		return { onChoose, onCancel };
	};

	it('opens in the application that is clicked', () => {
		const { onChoose } = open();
		fireEvent.click(screen.getByRole('button', { name: 'Calculator' }));
		expect(onChoose).toHaveBeenCalledWith(expect.objectContaining({ id: 'calc' }));
	});

	it('opens in the first match when Enter is pressed in the filter', () => {
		const { onChoose } = open();
		const filter = screen.getByRole('searchbox', { name: 'Find an application' });
		fireEvent.change(filter, { target: { value: 'calc' } });
		fireEvent.keyDown(filter, { key: 'Enter' });
		expect(onChoose).toHaveBeenCalledWith(expect.objectContaining({ id: 'calc' }));
	});

	it('says so when nothing matches, and Enter then chooses nothing', () => {
		const { onChoose } = open();
		const filter = screen.getByRole('searchbox');
		fireEvent.change(filter, { target: { value: 'zzz' } });
		fireEvent.keyDown(filter, { key: 'Enter' });
		expect(screen.getByText('No application matches.')).toBeTruthy();
		expect(onChoose).not.toHaveBeenCalled();
	});

	it('cancels from the button', () => {
		const { onCancel } = open();
		fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
		expect(onCancel).toHaveBeenCalled();
	});
});
