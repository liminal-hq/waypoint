// Verifies useSettings: defaults outside a provider, Rust's values inside one, and re-rendering only for the part read
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { createFakeSettingsClient } from '../services/fakeSettingsClient';
import { DEFAULT_SETTINGS } from '../services/settingsClient';
import { SettingsProvider, useSettings } from './SettingsContext';

afterEach(cleanup);

let renders = 0;

function Delay() {
	renders++;
	const ms = useSettings((settings) => settings.dnd.springLoadMs);
	return <output aria-label="delay">{ms}</output>;
}

describe('useSettings', () => {
	it('reads the defaults outside a provider', () => {
		render(<Delay />);
		expect(screen.getByLabelText('delay')).toHaveTextContent('600');
	});

	it('reads Rust’s settings once they arrive and follows changes from any window', async () => {
		const fake = createFakeSettingsClient({
			...DEFAULT_SETTINGS,
			dnd: { ...DEFAULT_SETTINGS.dnd, springLoadMs: 900 },
		});
		render(
			<SettingsProvider client={fake}>
				<Delay />
			</SettingsProvider>,
		);
		expect(await screen.findByText('900')).toBeInTheDocument();
		act(() => {
			fake.change({ ...DEFAULT_SETTINGS, dnd: { ...DEFAULT_SETTINGS.dnd, springLoadMs: 1200 } });
		});
		expect(screen.getByLabelText('delay')).toHaveTextContent('1200');
	});

	it('does not re-render for a change to a part it does not read', async () => {
		const fake = createFakeSettingsClient();
		render(
			<SettingsProvider client={fake}>
				<Delay />
			</SettingsProvider>,
		);
		await act(async () => {
			await fake.snapshot();
		});
		const before = renders;
		act(() => {
			fake.change({
				...DEFAULT_SETTINGS,
				general: { ...DEFAULT_SETTINGS.general, clickMode: 'single' },
			});
		});
		expect(renders).toBe(before);
	});

	it('stops following when the provider unmounts', async () => {
		const fake = createFakeSettingsClient();
		const { unmount } = render(
			<SettingsProvider client={fake}>
				<Delay />
			</SettingsProvider>,
		);
		await act(async () => {
			await fake.snapshot();
		});
		unmount();
		expect(() => fake.change(DEFAULT_SETTINGS)).not.toThrow();
	});
});
