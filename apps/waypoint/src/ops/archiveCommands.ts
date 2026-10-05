// Extract Here, Extract To…, Extract All and Compress…: the commands that make an archive's contents or an archive
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { JobSnapshot } from '@liminal-hq/waypoint-protocol/generated/JobSnapshot';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import { containerUri, defaultArchiveName, isArchiveEntry } from '../archives/archiveNames';
import type { ArchiveClient } from '../archives/archiveClient';
import { commandErrorMessage, lockOf } from '../archives/askPassphrase';
import type { ArchiveLock } from '../archives/lockModel';
import { PAGE_SIZE } from '../browse/listingModel';
import { isSelected, selectedCount, type Selection } from '../browse/selection';
import type { ListingSession } from '../browse/useListingSession';
import type { Answered } from '../connections/connectFlow';
import { t, tf, tn } from '../i18n/messages';
import type { JobRequest } from '../services/opsClient';
import type { VfsClient } from '../services/vfsClient';
import {
	compressRequest,
	extractRequest,
	leftOutCount,
	leftOutText,
	limitOf,
} from './archiveRequests';
import { pickCompression, type CompressChoice, type CompressOptions } from './compressStore';
import { pickDestination, type DestinationOptions } from './destinationStore';
import type { ConfirmSpec } from './fileCommands';
import { errorText } from './jobText';
import type { OpsHandle } from './opsStore';
import { commandErrorText } from './opsNotices';
import { problemText } from './problemModel';
import type { ArchiveSpec } from '@liminal-hq/waypoint-protocol/generated/ArchiveSpec';

/** What the archive commands borrow from the file commands they are part of. */
export interface ArchiveCommandDeps {
	ops: OpsHandle;
	vfs: VfsClient;
	windowLabel: string;
	say: (text: string) => void;
	confirm: (spec: ConfirmSpec) => Promise<boolean>;
	/** The pane the command acts on, `null` after saying why not. */
	readable: (session?: ListingSession | null) => ListingSession | null;
	/** The pane to write in, `null` after saying why not. */
	writable: (session?: ListingSession | null) => ListingSession | null;
	/** Puts a request on the queue and waits for its job; `null` when refused or not ended in time. */
	run: (request: JobRequest) => Promise<JobSnapshot | null>;
	/** Where passwords are given; without it a locked archive is reported as locked. */
	archives?: ArchiveClient | null;
	/** Asks the question a lock asks (the window's question dialogs). */
	ask?: (error: VfsError) => Promise<Answered | null>;
	pickDestination?: (options: DestinationOptions) => Promise<Location | null>;
	pickCompression?: (options: CompressOptions) => Promise<CompressChoice | null>;
}

export interface ArchiveCommands {
	extractHere(session?: ListingSession | null): Promise<void>;
	extractTo(session?: ListingSession | null): Promise<void>;
	/** Extracts the archive the pane is in (the whole of it, not what is shown). */
	extractAll(session?: ListingSession | null): Promise<void>;
	compress(session?: ListingSession | null): Promise<void>;
}

/** The entries of `selection` that are archives, in listing order. */
export async function selectedArchives(
	model: ListingSession['model'],
	selection: Selection,
): Promise<Entry[]> {
	const found: Entry[] = [];
	for (let position = 0; position < model.count; position += PAGE_SIZE) {
		for (const entry of await model.readRange(
			position,
			Math.min(model.count, position + PAGE_SIZE),
		)) {
			if (isSelected(selection, entry.id) && isArchiveEntry(entry)) found.push(entry);
		}
	}
	return found;
}

/** How many times a password is asked for in one command before it is given up. */
const MAX_PASSWORDS = 3;

export function createArchiveCommands(deps: ArchiveCommandDeps): ArchiveCommands {
	const { ops, vfs, say } = deps;
	const ask = deps.ask ?? (async () => null);

	/** Asks for the password the lock wants and gives it to Rust; `false` when the person cancels or it could not be given. */
	const unlock = async (lock: ArchiveLock): Promise<boolean> => {
		if (!deps.archives) {
			say(t('files.extract.stillLocked'));
			return false;
		}
		const answered = await ask({
			kind: 'authRequired',
			location: lock.location,
			prompt: { kind: 'passphrase', subject: lock.location.display },
		});
		if (answered?.answer.kind !== 'passphrase') return false;
		try {
			await deps.archives.unlock(lock.location, answered.answer.passphrase);
			return true;
		} catch (error) {
			say(tf('files.extract.unlockFailed', { reason: commandErrorMessage(error) }));
			return false;
		}
	};

	/**
	 * Plans `request` and settles what planning finds before anything is written: an archive that
	 * needs its password asks for it, and one past the limits is confirmed. Resolves to the request
	 * to run (with `allowLarge` when the person went ahead), or `null` when it was given up.
	 */
	const prepare = async (request: JobRequest, name: string): Promise<JobRequest | null> => {
		let current = request;
		for (let attempt = 0; attempt <= MAX_PASSWORDS; attempt++) {
			try {
				const preview = await ops.client.plan(current);
				const left = leftOutCount(preview);
				if (left > 0) say(leftOutText(left));
				return current;
			} catch (failure) {
				const lock = lockOf(failure);
				if (lock) {
					if (!(await unlock(lock))) return null;
					continue;
				}
				const limit = limitOf(failure);
				if (limit && current.archive?.kind === 'extract' && !current.archive.allowLarge) {
					const reason = problemText(limit).details[0] ?? '';
					const ok = await deps.confirm({
						title: t('files.extract.limit.title'),
						message: tf('files.extract.limit.message', { name, reason }),
						confirmLabel: t('files.extract.limit.action'),
						danger: true,
					});
					if (!ok) return null;
					const spec: ArchiveSpec = { ...current.archive, allowLarge: true };
					current = { ...current, archive: spec };
					continue;
				}
				say(tf('files.failed', { reason: commandErrorText(failure) }));
				return null;
			}
		}
		say(t('files.extract.stillLocked'));
		return null;
	};

	/** Runs a prepared request; a job that fails for want of the password asks for it and goes again. */
	const execute = async (request: JobRequest): Promise<JobSnapshot | null> => {
		for (let attempt = 0; attempt <= MAX_PASSWORDS; attempt++) {
			const job = await deps.run(request);
			if (job?.state.state !== 'failed') return job;
			const lock = lockOf(job.state.error);
			if (!lock) {
				say(tf('files.failed', { reason: errorText(job.state.error) }));
				return job;
			}
			// The failed attempt leaves nothing; take it off the queue before asking again.
			void ops.client.dismiss(job.id).catch(() => {});
			if (!(await unlock(lock))) return null;
		}
		say(t('files.extract.stillLocked'));
		return null;
	};

	const extractLocations = async (
		archives: Location[],
		destination: Location | null,
		name: string,
		options: { layout?: 'folder'; keepBoth?: boolean } = {},
	): Promise<void> => {
		const request = await prepare(
			extractRequest(archives, destination, deps.windowLabel, options),
			name,
		);
		if (request) await execute(request);
	};

	/** The archive files selected in `found`, as locations; says so when none is an archive. */
	const chosenArchives = async (found: ListingSession): Promise<Location[]> => {
		const { model, store } = found;
		const entries = await selectedArchives(model, store.getState().selection);
		if (entries.length === 0) {
			say(t('files.extract.nothing'));
			return [];
		}
		try {
			return await Promise.all(entries.map((entry) => vfs.entryLocation(model.handle, entry.id)));
		} catch (error) {
			say(tf('files.failed', { reason: commandErrorText(error) }));
			return [];
		}
	};

	const archiveName = (locations: Location[]): string =>
		locations.length === 1 ? (locations[0]?.display.split(/[\\/]/).pop() ?? '') : '';

	return {
		async extractHere(session) {
			const found = deps.writable(session);
			if (!found) return;
			const archives = await chosenArchives(found);
			if (archives.length > 0) await extractLocations(archives, null, archiveName(archives));
		},

		async extractTo(session) {
			const found = deps.readable(session);
			if (!found) return;
			const archives = await chosenArchives(found);
			if (archives.length === 0) return;
			const choose = deps.pickDestination ?? pickDestination;
			const destination = await choose({
				title: tn('destination.title.extract', archives.length),
				confirmLabel: t('destination.extract'),
				base: found.model.location,
			});
			if (destination) await extractLocations(archives, destination, archiveName(archives));
		},

		async extractAll(session) {
			const found = deps.readable(session);
			if (!found) return;
			const container = containerUri(found.model.location);
			if (!container) return;
			let archive: Location;
			try {
				archive = await vfs.parseLocation(container, found.model.location);
			} catch (error) {
				say(tf('files.failed', { reason: commandErrorText(error) }));
				return;
			}
			const name = archive.display.split(/[\\/]/).pop() ?? archive.display;
			// Straight into a new folder beside the archive, named after it; a name already taken gets
			// the next free one (`name (2)`), so nothing is merged into or replaced.
			await extractLocations([archive], null, name, { layout: 'folder', keepBoth: true });
		},

		async compress(session) {
			const found = deps.writable(session);
			if (!found) return;
			const { model, store } = found;
			const selection = store.getState().selection;
			if (selectedCount(selection, model.count) === 0) {
				say(t('files.compress.nothing'));
				return;
			}
			// Names of what is chosen, for the starting name of the archive.
			const names: string[] = [];
			const locations: Location[] = [];
			for (let position = 0; position < model.count; position += PAGE_SIZE) {
				for (const entry of await model.readRange(
					position,
					Math.min(model.count, position + PAGE_SIZE),
				)) {
					if (!isSelected(selection, entry.id)) continue;
					names.push(entry.name);
					try {
						locations.push(await vfs.entryLocation(model.handle, entry.id));
					} catch (error) {
						say(tf('files.failed', { reason: commandErrorText(error) }));
						return;
					}
				}
			}
			const choose = deps.pickCompression ?? pickCompression;
			const choice = await choose({
				name: defaultArchiveName(names, t('compress.default')),
				count: locations.length,
			});
			if (!choice) return;
			const request = compressRequest(
				locations,
				model.location,
				choice.name,
				choice.format,
				deps.windowLabel,
			);
			try {
				await ops.client.plan(request);
			} catch (failure) {
				say(tf('files.failed', { reason: commandErrorText(failure) }));
				return;
			}
			const job = await deps.run(request);
			if (job?.state.state === 'failed') {
				say(tf('files.failed', { reason: errorText(job.state.error) }));
			}
		},
	};
}
