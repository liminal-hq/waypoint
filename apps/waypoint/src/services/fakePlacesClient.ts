// An in-memory PlacesClient with the real service's semantics, for tests and the `?demo` window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Favourite } from '@liminal-hq/waypoint-protocol/generated/Favourite';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { Place } from '@liminal-hq/waypoint-protocol/generated/Place';
import type { Places } from '@liminal-hq/waypoint-protocol/generated/Places';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import { fileLocation, pathOf } from './fakeVfsClient';
import { PlacesChangeEmitter, type PlacesClient } from './placesClient';
import type { Unsubscribe } from './vfsClient';

/** The fixed places of a home folder, in the order the plugin returns them. */
export function fakePlaces(home: string): Place[] {
	const folder = (kind: Place['kind'], label: string, path: string): Place => ({
		kind,
		label,
		location: fileLocation(path),
	});
	return [
		folder('home', 'Home', home),
		folder('desktop', 'Desktop', `${home}/Desktop`),
		folder('documents', 'Documents', `${home}/Documents`),
		folder('downloads', 'Downloads', `${home}/Downloads`),
		folder('pictures', 'Pictures', `${home}/Pictures`),
		folder('music', 'Music', `${home}/Music`),
		folder('videos', 'Videos', `${home}/Videos`),
	];
}

export interface FakePlacesOptions {
	places?: Place[];
	favourites?: Favourite[];
}

/**
 * Follows `waypoint-vfs`'s bookmarks semantics: a label defaults to the folder's own name; adding
 * a pinned folder, or removing, renaming or moving one that is not, changes nothing (and still
 * resolves); a move lands the Favourite at index `to` of the list without it, clamped to the end;
 * whitespace in a label collapses and a blank label clears it. The same mutation reaches every
 * listener, as the real client does.
 */
export class FakePlacesClient implements PlacesClient {
	private places: Place[];
	private favourites: Array<{ label: string | null; location: Location }>;
	private changes = new PlacesChangeEmitter();
	/** Every call made, in order, for tests to check what the UI asked for. */
	readonly calls: string[] = [];
	private failure: VfsError | null = null;

	constructor(options: FakePlacesOptions = {}) {
		this.places = options.places ?? fakePlaces('/home/test');
		this.favourites = (options.favourites ?? []).map((favourite) => ({
			label: favourite.label,
			location: favourite.location,
		}));
	}

	/** Makes the next mutation reject with `error` (once), for testing how the UI reports failure. */
	failNext(error: VfsError): void {
		this.failure = error;
	}

	async list(): Promise<Places> {
		this.calls.push('list');
		return this.snapshot();
	}

	async addFavourite(location: Location, label?: string): Promise<Places> {
		this.calls.push(`add ${location.uri}`);
		this.maybeFail();
		if (this.indexOf(location) < 0) {
			this.favourites.push({ label: cleanLabel(label ?? null), location });
		}
		return this.changes.emit(this.snapshot());
	}

	async removeFavourite(location: Location): Promise<Places> {
		this.calls.push(`remove ${location.uri}`);
		this.maybeFail();
		const at = this.indexOf(location);
		if (at >= 0) this.favourites.splice(at, 1);
		return this.changes.emit(this.snapshot());
	}

	async renameFavourite(location: Location, label: string | null): Promise<Places> {
		this.calls.push(`rename ${location.uri} ${label ?? ''}`);
		this.maybeFail();
		const favourite = this.favourites[this.indexOf(location)];
		if (favourite) favourite.label = cleanLabel(label);
		return this.changes.emit(this.snapshot());
	}

	async moveFavourite(location: Location, to: number): Promise<Places> {
		this.calls.push(`move ${location.uri} ${to}`);
		this.maybeFail();
		const at = this.indexOf(location);
		if (at >= 0) {
			const [favourite] = this.favourites.splice(at, 1);
			this.favourites.splice(Math.max(0, Math.min(to, this.favourites.length)), 0, favourite!);
		}
		return this.changes.emit(this.snapshot());
	}

	onChange(listener: (places: Places) => void): Unsubscribe {
		return this.changes.subscribe(listener);
	}

	private indexOf(location: Location): number {
		return this.favourites.findIndex((favourite) => favourite.location.uri === location.uri);
	}

	private maybeFail(): void {
		const failure = this.failure;
		this.failure = null;
		if (failure) throw failure;
	}

	private snapshot(): Places {
		return structuredClone({
			places: this.places,
			favourites: this.favourites.map((favourite): Favourite => ({
				label: favourite.label ?? defaultLabel(favourite.location),
				location: favourite.location,
			})),
		});
	}
}

function cleanLabel(label: string | null): string | null {
	const collapsed = label?.split(/\s+/).filter(Boolean).join(' ') ?? '';
	return collapsed === '' ? null : collapsed;
}

function defaultLabel(location: Location): string {
	const path = pathOf(location);
	return path.split('/').filter(Boolean).pop() ?? location.display;
}
