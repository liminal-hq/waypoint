// Verifies the window's OS title follows what the window holds, and that a missing chrome or `setTitle` is no error
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { WindowChromeProvider } from '@liminal-hq/waypoint-chrome/WindowChromeProvider/WindowChromeProvider';
import type { WindowControls } from '@liminal-hq/waypoint-chrome/TitleBar/windowControls';
import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { useWindowTitle } from './useWindowTitle';

function Title({ title }: { title: string }) {
	useWindowTitle(title);
	return null;
}

function controlsWith(setTitle?: WindowControls['setTitle']): WindowControls {
	return {
		minimize: () => {},
		toggleMaximize: () => {},
		close: () => {},
		setAlwaysOnTop: () => {},
		isMaximized: () => false,
		onMaximizedChange: () => () => {},
		setTitle,
	};
}

afterEach(cleanup);

describe('useWindowTitle', () => {
	it('sets the OS title and the document title, and follows a new title', () => {
		const setTitle = vi.fn();
		const { rerender } = render(
			<WindowChromeProvider controls={controlsWith(setTitle)}>
				<Title title="Pictures" />
			</WindowChromeProvider>,
		);
		expect(setTitle).toHaveBeenLastCalledWith('Pictures');
		expect(document.title).toBe('Pictures');
		rerender(
			<WindowChromeProvider controls={controlsWith(setTitle)}>
				<Title title="Downloads" />
			</WindowChromeProvider>,
		);
		expect(setTitle).toHaveBeenLastCalledWith('Downloads');
	});

	it('does nothing to the OS where there is no chrome or the controls cannot set a title', () => {
		render(<Title title="Alone" />);
		expect(document.title).toBe('Alone');
		render(
			<WindowChromeProvider controls={controlsWith()}>
				<Title title="No setter" />
			</WindowChromeProvider>,
		);
		expect(document.title).toBe('No setter');
	});

	it('swallows a setter that throws', () => {
		const setTitle = vi.fn(() => {
			throw new Error('not a function');
		});
		render(
			<WindowChromeProvider controls={controlsWith(setTitle)}>
				<Title title="Throws" />
			</WindowChromeProvider>,
		);
		expect(setTitle).toHaveBeenCalledWith('Throws');
		expect(document.title).toBe('Throws');
	});

	it('swallows a refusal from the window manager', async () => {
		const setTitle = vi.fn().mockRejectedValue(new Error('refused'));
		render(
			<WindowChromeProvider controls={controlsWith(setTitle)}>
				<Title title="Refused" />
			</WindowChromeProvider>,
		);
		await Promise.resolve();
		expect(setTitle).toHaveBeenCalledWith('Refused');
	});
});
