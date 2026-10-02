// What Overview knows about the size of the home folder: nothing yet, until the directory-size scan reports
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/** The size of the home folder, as far as it has been measured. */
export type HomeMeasure =
	| { status: 'notMeasured' }
	| { status: 'measuring' }
	| {
			status: 'done';
			/** The bytes the home folder holds. */
			bytes: number;
			/** When the measurement finished, in milliseconds since the Unix epoch; `null` when it is unknown. */
			asOfMs: number | null;
	  };

export const NOT_MEASURED: HomeMeasure = { status: 'notMeasured' };

/**
 * The hook the directory-size scan (A70) plugs into. Until the scan exists it reports that Home
 * has not been measured, so the headline stat says "Not measured yet" rather than a number it
 * cannot stand behind. The scan's slice replaces this body and nothing else in Overview changes.
 */
export function useHomeMeasure(): HomeMeasure {
	return NOT_MEASURED;
}
