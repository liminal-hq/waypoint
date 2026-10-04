// An in-memory GitClient: repositories a test describes, with the plugin's contract for ids, revisions and events
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { GitSummary } from '@liminal-hq/waypoint-protocol/generated/GitSummary';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { GitBadge, GitChanged, GitClient, GitWatch } from './gitClient';
import type { Unsubscribe } from './vfsClient';

/** A clean repository on `main` with no upstream: what a test changes from. */
export function cleanSummary(overrides: Partial<GitSummary> = {}): GitSummary {
	return {
		headKind: 'branch',
		head: 'main',
		commit: 'a1b2c3d4e5f60718293a4b5c6d7e8f9012345678',
		upstream: null,
		ahead: null,
		behind: null,
		distanceCapped: false,
		staged: 0,
		unstaged: 0,
		untracked: 0,
		conflicted: 0,
		operation: null,
		...overrides,
	};
}

export interface FakeRepository {
	/** The repository's working folder; a watched folder is in it when its uri starts with `root.uri`. */
	root: Location;
	name: string;
	summary: GitSummary;
}

export interface FakeGitClient extends GitClient {
	/** The watches now open, by id. */
	readonly watching: ReadonlyMap<number, Location>;
	/** Every `watch` that was asked, in order, whether it found a repository or not. */
	readonly watched: Location[];
	/** Changes a repository as the plugin would: its summary is replaced and every watch of it is told. */
	change(root: string, summary: GitSummary): void;
	/** Sends an event as the plugin would, without changing anything held (a late or repeated one). */
	emit(changed: GitChanged): void;
	/** Holds the next `watch` reply until `release` is called. */
	holdWatch(): { release(): void };
}

/** A client over `repositories` (found by the folder's uri starting with the root's) and the sidebar's `badges`. */
export function createFakeGitClient(
	repositories: FakeRepository[] = [],
	badges: GitBadge[] = [],
): FakeGitClient {
	let nextId = 1;
	const watching = new Map<number, Location>();
	const revisions = new Map<number, number>();
	const watched: Location[] = [];
	const listeners = new Set<(changed: GitChanged) => void>();
	let hold: Promise<void> | null = null;

	const repositoryOf = (location: Location) =>
		repositories.find(
			(repo) => location.uri === repo.root.uri || location.uri.startsWith(`${repo.root.uri}/`),
		);

	return {
		watching,
		watched,
		async watch(location) {
			watched.push(location);
			if (hold) await hold;
			const repo = repositoryOf(location);
			if (!repo) return null;
			const id = nextId++;
			watching.set(id, location);
			revisions.set(id, 1);
			const reply: GitWatch = {
				id,
				root: repo.root,
				name: repo.name,
				summary: repo.summary,
				revision: 1,
			};
			return reply;
		},
		async unwatch(id) {
			watching.delete(id);
			revisions.delete(id);
		},
		async badges(locations) {
			const asked = new Set(locations.map((location) => location.uri));
			return badges.filter((badge) => asked.has(badge.uri));
		},
		onChanged(listener): Unsubscribe {
			listeners.add(listener);
			return () => {
				listeners.delete(listener);
			};
		},
		change(root, summary) {
			const repo = repositories.find((candidate) => candidate.root.uri === root);
			if (repo) repo.summary = summary;
			for (const [id, location] of watching) {
				if (repositoryOf(location)?.root.uri !== root) continue;
				const revision = (revisions.get(id) ?? 1) + 1;
				revisions.set(id, revision);
				for (const listener of listeners) listener({ id, revision, summary });
			}
		},
		emit(changed) {
			for (const listener of listeners) listener(changed);
		},
		holdWatch() {
			let release = () => {};
			hold = new Promise<void>((resolve) => {
				release = () => {
					hold = null;
					resolve();
				};
			});
			return { release };
		},
	};
}
