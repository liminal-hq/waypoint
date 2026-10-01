// What the Trash view needs from the file system and operations plugins: how full the Trash is, and the jobs that change it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { JobId } from '@liminal-hq/waypoint-protocol/generated/JobId';
import type { JobRequest } from '@liminal-hq/waypoint-protocol/generated/JobRequest';
import type { OpsEvent } from '@liminal-hq/waypoint-protocol/generated/OpsEvent';
import type { TrashInfo } from '@liminal-hq/waypoint-protocol/generated/TrashInfo';
import type { Unsubscribe } from '../services/vfsClient';

/**
 * The seam between the Trash view and the plugins behind it, injected so the view is tested
 * without them. Rust owns the Trash (A53) and the queue (A3); this only asks and listens.
 *
 * Every method rejects when the plugin refuses; a job that is accepted and then fails reports that
 * through `onEvent`, never through `submit`. A job that waits for an answer (a name that is taken,
 * a folder that is gone) is answered through the queue's own resolver, not here.
 */
export interface TrashClient {
	/** Whether the Trash can be browsed here, why not, and how many items it holds. */
	getInfo(): Promise<TrashInfo>;
	/** Queues a job (`Restore`, `Delete` or `EmptyTrash`) and resolves with its id. */
	submit(request: JobRequest): Promise<JobId>;
	/** Hears every change to the queue, from every window. */
	onEvent(listener: (event: OpsEvent) => void): Unsubscribe;
}
