// What the Operations rows show while the operations settings are not readable
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { OpsSettings } from '../services/opsClient';

/** Mirrors `OpsSettings::default` in `crates/waypoint-ops`; shown, never saved. */
export const DEFAULT_OPS: OpsSettings = {
	concurrency: 2,
	verifyAfterCopy: false,
	verifyAlgorithm: 'blake3',
	confirmTrash: false,
	undoDepth: 50,
	trashExpiryDays: null,
	speedLimitBps: null,
};

/** The speed, in MB/s, a newly switched-on limit starts at. */
export const DEFAULT_SPEED_LIMIT_MBPS = 10;

/** Bytes in the megabyte the speed limit is set in, as the sizes shown elsewhere count them. */
export const BYTES_PER_MB = 1_000_000;

/** The days a newly switched-on Trash sweep waits (the Trash specification's own default). */
export const DEFAULT_TRASH_EXPIRY_DAYS = 30;
