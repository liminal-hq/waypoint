// The system's hour cycle for every view below, kept current as the setting changes
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useContext, useEffect, useState, type ReactNode } from 'react';
import type { HourCycle, TimeFormatClient } from '../services/timeFormatClient';

const HourCycleContext = createContext<HourCycle | undefined>(undefined);

interface TimeFormatProviderProps {
	/** Without a client the hour cycle stays `undefined` and `Intl` decides, as before. */
	client?: TimeFormatClient;
	children: ReactNode;
}

/**
 * Reads the system's hour cycle once and follows its changes, so every view of the window shares
 * one subscription. It subscribes before it reads, and a change that arrives first wins over the
 * read that was already in flight.
 */
export function TimeFormatProvider({ client, children }: TimeFormatProviderProps) {
	const [hourCycle, setHourCycle] = useState<HourCycle | undefined>(undefined);
	useEffect(() => {
		if (!client) return;
		let live = true;
		let changed = false;
		const unsubscribe = client.onChange((next) => {
			changed = true;
			if (live) setHourCycle(next);
		});
		void client.get().then((initial) => {
			if (live && !changed) setHourCycle(initial);
		});
		return () => {
			live = false;
			unsubscribe();
		};
	}, [client]);
	return <HourCycleContext.Provider value={hourCycle}>{children}</HourCycleContext.Provider>;
}

/**
 * `'h23'` for a 24-hour clock and `'h12'` for a 12-hour one. `undefined` while the setting loads,
 * or when it is unknown, means: let `Intl` decide from the locale.
 */
export function useHourCycle(): HourCycle | undefined {
	return useContext(HourCycleContext);
}
