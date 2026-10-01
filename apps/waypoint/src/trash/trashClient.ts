// What the Trash view needs from the file system and operations plugins: how full the Trash is, and the jobs that change it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ConflictPolicy } from '@liminal-hq/waypoint-protocol/generated/ConflictPolicy';
import type { Decision } from '@liminal-hq/waypoint-protocol/generated/Decision';
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
 * through `onEvent`, never through `submit`.
 */
export interface TrashClient {
	/** Whether the Trash can be browsed here, why not, and how many items it holds. */
	getInfo(): Promise<TrashInfo>;
	/** Queues a job (`Restore`, `Delete` or `EmptyTrash`) and resolves with its id. */
	submit(request: JobRequest): Promise<JobId>;
	/** Answers the clashes a restore waits on: one policy for every clash the job has. */
	resolveConflicts(job: JobId, policy: ConflictPolicy): Promise<void>;
	/** Answers the error a job waits on. */
	resolveError(job: JobId, decision: Decision): Promise<void>;
	/** Stops a job, which a window does when its question is dismissed. */
	cancel(job: JobId): Promise<void>;
	/** Hears every change to the queue, from every window. */
	onEvent(listener: (event: OpsEvent) => void): Unsubscribe;
}
