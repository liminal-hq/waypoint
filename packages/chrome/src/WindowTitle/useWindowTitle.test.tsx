// Verifies one title call reaches the title bar, the host's title and the page's, and the rules for promises
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { TitleBarTitle } from '../TitleBar/TitleBarTitle';
import type { WindowControls } from '../TitleBar/windowControls';
import { WindowChromeProvider } from '../WindowChromeProvider/WindowChromeProvider';
import { useWindowTitle } from './useWindowTitle';

type Title = string | Promise<string> | undefined;

function Setter({ title }: { title: Title }) {
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

function deferred() {
	let resolve!: (value: string) => void;
	let reject!: (reason: unknown) => void;
	const promise = new Promise<string>((res, rej) => {
		resolve = res;
		reject = rej;
	});
	return { promise, resolve, reject };
}

function Harness({
	title,
	controls,
	formatTitle,
}: {
	title: Title | null;
	controls: WindowControls;
	formatTitle?: (title: string) => string;
}) {
	return (
		<WindowChromeProvider controls={controls} formatTitle={formatTitle}>
			<TitleBarTitle fallback="Fallback" />
			{title !== null && <Setter title={title} />}
		</WindowChromeProvider>
	);
}

afterEach(() => {
	cleanup();
	document.title = '';
});

describe('useWindowTitle', () => {
	it('sets the title bar text, the OS title and the document title, and follows a new title', () => {
		const setTitle = vi.fn();
		const controls = controlsWith(setTitle);
		const { rerender } = render(<Harness title="Pictures" controls={controls} />);
		expect(screen.getByText('Pictures')).toBeInTheDocument();
		expect(setTitle).toHaveBeenLastCalledWith('Pictures');
		expect(document.title).toBe('Pictures');
		rerender(<Harness title="Downloads" controls={controls} />);
		expect(screen.getByText('Downloads')).toBeInTheDocument();
		expect(setTitle).toHaveBeenLastCalledWith('Downloads');
		expect(document.title).toBe('Downloads');
	});

	it('shapes only the title bar text with formatTitle', () => {
		const setTitle = vi.fn();
		render(
			<Harness
				title="Trash"
				controls={controlsWith(setTitle)}
				formatTitle={(title) => `App — ${title}`}
			/>,
		);
		expect(screen.getByText('App — Trash')).toBeInTheDocument();
		expect(setTitle).toHaveBeenLastCalledWith('Trash');
		expect(document.title).toBe('Trash');
	});

	it('keeps the previous title while a promise is pending, then shows its value', async () => {
		const setTitle = vi.fn();
		const controls = controlsWith(setTitle);
		const { rerender } = render(<Harness title="Before" controls={controls} />);
		const next = deferred();
		rerender(<Harness title={next.promise} controls={controls} />);
		expect(screen.getByText('Before')).toBeInTheDocument();
		await act(async () => next.resolve('After'));
		expect(screen.getByText('After')).toBeInTheDocument();
		expect(setTitle).toHaveBeenLastCalledWith('After');
		expect(document.title).toBe('After');
	});

	it('ignores a promise that a newer call has overtaken', async () => {
		const setTitle = vi.fn();
		const controls = controlsWith(setTitle);
		const slow = deferred();
		const { rerender } = render(<Harness title={slow.promise} controls={controls} />);
		rerender(<Harness title="Newer" controls={controls} />);
		await act(async () => slow.resolve('Stale'));
		expect(screen.getByText('Newer')).toBeInTheDocument();
		expect(setTitle).not.toHaveBeenCalledWith('Stale');
		expect(document.title).toBe('Newer');
	});

	it('ignores a rejection', async () => {
		const setTitle = vi.fn();
		const controls = controlsWith(setTitle);
		const { rerender } = render(<Harness title="Kept" controls={controls} />);
		const failing = deferred();
		rerender(<Harness title={failing.promise} controls={controls} />);
		await act(async () => failing.reject(new Error('no name')));
		expect(screen.getByText('Kept')).toBeInTheDocument();
		expect(document.title).toBe('Kept');
	});

	it('leaves the title as it was for undefined, and after the setter unmounts', () => {
		const controls = controlsWith(vi.fn());
		const { rerender } = render(<Harness title="Held" controls={controls} />);
		rerender(<Harness title={undefined} controls={controls} />);
		expect(screen.getByText('Held')).toBeInTheDocument();
		rerender(<Harness title={null} controls={controls} />);
		expect(screen.getByText('Held')).toBeInTheDocument();
		expect(document.title).toBe('Held');
	});

	it('does nothing without a chrome provider', () => {
		document.title = 'Untouched';
		render(<Setter title="Alone" />);
		expect(document.title).toBe('Untouched');
	});

	it('still shows the title where the controls cannot set one', () => {
		render(<Harness title="No setter" controls={controlsWith()} />);
		expect(screen.getByText('No setter')).toBeInTheDocument();
		expect(document.title).toBe('No setter');
	});

	it('swallows a setter that throws', () => {
		const setTitle = vi.fn(() => {
			throw new Error('not a function');
		});
		render(<Harness title="Throws" controls={controlsWith(setTitle)} />);
		expect(setTitle).toHaveBeenCalledWith('Throws');
		expect(screen.getByText('Throws')).toBeInTheDocument();
		expect(document.title).toBe('Throws');
	});

	it('swallows a refusal from the window manager', async () => {
		const setTitle = vi.fn().mockRejectedValue(new Error('refused'));
		render(<Harness title="Refused" controls={controlsWith(setTitle)} />);
		await Promise.resolve();
		expect(setTitle).toHaveBeenCalledWith('Refused');
		expect(screen.getByText('Refused')).toBeInTheDocument();
	});
});
