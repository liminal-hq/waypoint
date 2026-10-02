// What dragging files inside the window means: sources, targets, spring-loading, the default action rule and the drop
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { IconGroup } from '@liminal-hq/waypoint-protocol/generated/IconGroup';
import type { OpsError } from '@liminal-hq/waypoint-protocol/generated/OpsError';
import { isSelected, selectedCount, selectOnly } from '../browse/selection';
import type { ListingSession } from '../browse/useListingSession';
import { t, tf } from '../i18n/messages';
import { normaliseUri } from '../ops/clipboardRules';
import { selectionSpec } from '../ops/fileCommands';
import type {
	JobRequest,
	ListingHandle,
	Location,
	OpsCommandError,
	PlanPreview,
	SelectionSpec,
} from '../services/opsClient';
import {
	nativeDndErrorKind,
	type DragAction,
	type DragEnded,
	type OutboundRequest,
	type OutboundStarted,
} from '../services/nativeDndClient';
import {
	createDragSession,
	type DragClock,
	type DragControl,
	type DragPill,
	type DragHandlers,
	type DragSession,
	type Point,
} from './dragSession';
import { modifiersOf, NO_MODIFIERS, pickerVerbs, type DropModifiers } from './dropAction';
import {
	clearMark,
	dropSpotAt,
	edgeScrollStep,
	markOver,
	parseFolderRef,
	SPRING_ATTRIBUTE,
	type DropSpot,
	type HitRoot,
} from './dropTargets';
import {
	blockedText,
	canLinkSource,
	dragText,
	evaluateTarget,
	isSourceFolder,
	pillFor,
	sameFileTarget,
	subjectText,
	type FileDragSource,
	type FileDropTarget,
	type PlanFact,
} from './fileDragModel';
import { externalSource, sameUris } from './nativeDropModel';
import type { DropActionRule } from '@liminal-hq/waypoint-protocol/generated/DropActionRule';

/** The pointer travels this far before a press on a row becomes a drag (the same 4 px as a tab's). */
export const FILE_DRAG_START_PX = 4;

/** How long the pointer rests on a target before the planner is asked about it, so sweeping across rows asks about none. */
export const PLAN_REST_MS = 120;

/** A spring that has just opened something waits for the pointer to move this far before it opens another. */
export const SPRING_JITTER_PX = 6;

/** A selection of up to this many items has its locations resolved as the drag begins, so leaving the window can hand over at once. */
export const PREFETCH_LIMIT = 500;

/** How long a finished outbound drag is still remembered: the platform may deliver its drop after it has ended. */
export const OWN_DRAG_MS = 1500;

/** The longest this window's system drag is remembered without hearing that it ended (`drag-ended` is lost or never comes). */
export const OWN_DRAG_MAX_MS = 5 * 60 * 1000;

/** How often a drag near the edge of a list scrolls it. */
const SCROLL_TICK_MS = 16;

/** Set on the root element while a file drag runs, so the stylesheet can stop text selection and set the cursor. */
export const FILE_DRAG_ATTRIBUTE = 'data-file-drag';

/** A press on a file row that may become a drag. */
export interface FileDragPress {
	pointerId: number;
	clientX: number;
	clientY: number;
	/** 0 for the primary button, 2 for the secondary one, which opens the picker on release. */
	button: number;
	/** The element that gets pointer capture: the list, which outlives the row. */
	element: Element | null;
	session: ListingSession;
	position: number;
	entry: Entry;
	/** The pane the row is in. */
	tab: number | null;
	modifiers: DropModifiers;
}

/** What the picker needs to offer a choice and carry it out. */
export interface PickerRequest {
	position: Point;
	what: string;
	target: FileDropTarget;
	verbs: Array<'copy' | 'move' | 'link'>;
	/** Carries out the verb chosen. */
	choose(verb: 'copy' | 'move' | 'link'): void;
	/** The picker was dismissed. */
	cancel(): void;
}

/** What a drag that leaves the window needs to become a system drag (`NativeDndClient` and the operations plugin). */
export interface OutboundDeps {
	/** The plugin can start a system drag here. */
	available(): boolean;
	/** The pointer is outside the visible window. */
	outside(point: Point): boolean;
	/** The lossless locations of a selection of a listing this window opened. */
	resolve(handle: ListingHandle, spec: SelectionSpec): Promise<Location[]>;
	start(request: OutboundRequest): Promise<OutboundStarted>;
}

/** Files that came from outside the window, as the plugin reports them. */
export interface NativeFiles {
	/** The `file:` locations, in the order given. */
	files: Location[];
	point: Point;
	modifiers: DropModifiers;
}

/** The feed of a drag the system runs over the window: where it is, and how it ends. */
export interface NativeFeed {
	move(point: Point, modifiers: DropModifiers): void;
	/** `selfDrop`: the platform says the files are the ones this process offered. */
	drop(point: Point, modifiers: DropModifiers, selfDrop: boolean): void | Promise<void>;
	/** The files left the window, or the system ended the drag, without a drop. */
	leave(): void;
}

export interface OpenFoldersRequest {
	source: FileDragSource;
	target: 'plus' | 'chip';
	/** The group a chip belongs to. */
	group: number | null;
	/** Alt was held: open a split pair rather than tabs. */
	split: boolean;
}

/** Everything the drag reads from or asks of the rest of the window. */
export interface FileDragDeps {
	rule(): DropActionRule;
	springMs(): number;
	/** The live region. */
	announce(text: string): void;
	/** A message that is shown and read out (the status bar's toast). */
	say(text: string): void;
	/** The window's label, which a job carries as its `originWindow`; `null` where there is no queue. */
	windowLabel(): string | null;
	plan(request: JobRequest): Promise<PlanPreview>;
	entryLocation(handle: ListingHandle, entry: number): Promise<Location>;
	/** What a pane shows and whether it can be written to. */
	pane(tab: number): { location: Location; readOnly: boolean } | null;
	tabLocation(tab: number): Location | null;
	activeTab(): number | null;
	navigate(tab: number, location: Location): Promise<void>;
	back(tab: number): Promise<void>;
	activate(tab: number): Promise<void>;
	/** Holds a pane's listing open for the length of the drag; `null` when it has none. */
	retain(tab: number): (() => void) | null;
	transfer(
		kind: 'copy' | 'move' | 'link',
		session: ListingSession,
		destination: Location,
	): Promise<void>;
	/** Copies, moves or links files that came from outside the window's listings. */
	transferLocations?(
		kind: 'copy' | 'move' | 'link',
		items: Location[],
		destination: Location,
	): Promise<void>;
	moveToTrash(session: ListingSession): Promise<void>;
	openFolders(request: OpenFoldersRequest): Promise<void>;
	openPicker(request: PickerRequest): void;
	trashAvailable(): boolean;
	/** Hands a drag that leaves the window to the system; without it such a drag stays in the page. */
	outbound?: OutboundDeps;
	/** The document's hit test, or a stand-in. */
	hit?: HitRoot;
	clock?: DragClock;
	reducedMotion?: () => boolean;
	root?: () => HTMLElement | null;
}

export interface FileDrag {
	session: DragSession<FileDragSource, FileDropTarget>;
	/** Starts tracking a press on a row; false when a drag is already running, or the row cannot be dragged. */
	press(press: FileDragPress): boolean;
	/**
	 * Holds a context menu back while a right-button press may still become a drag. The press
	 * ending without a drag runs `open`; a drag drops it. False when there is no such press, so the
	 * caller opens the menu itself.
	 */
	deferMenu(open: () => void): boolean;
	/**
	 * Starts the drag of files the system is dragging over the window (from another application or
	 * window, or this window's own drag coming back); `null` when a drag is already running or the
	 * window has no queue. The caller feeds the returned drag the system's events.
	 */
	beginNative(input: NativeFiles): NativeFeed | null;
	/** The system's drag that this window started has ended, however it ended. */
	dragEnded(event: DragEnded): void;
	/** The document saw the pointer leave the window during a press, which hands the drag to the system. */
	leftWindow(): void;
	dispose(): void;
}

const realClock: DragClock = {
	setTimeout: (handler, ms) => globalThis.setTimeout(handler, ms),
	clearTimeout: (handle) => globalThis.clearTimeout(handle as ReturnType<typeof setTimeout>),
};

interface Spring {
	/** The spot is still within what the spring opened, so it stays open. */
	stays(spot: DropSpot): boolean;
	revert(): Promise<void>;
}

const NO_OPTIONS = { conflict: null, verify: null } as const;

const SPRING_KINDS = new Set(['folder', 'place', 'tab']);

function commandFailure(error: unknown): OpsError | null {
	const inner = (error as Partial<OpsCommandError> | null)?.error ?? error;
	const kind = (inner as { kind?: unknown } | null)?.kind;
	return typeof kind === 'string' ? (inner as OpsError) : null;
}

/** The icons of the stack: the pressed row's, then plain pages for the rest, up to three. */
function stackGroups(first: IconGroup, count: number): IconGroup[] {
	return [first, ...Array.from({ length: Math.min(count, 3) - 1 }, (): IconGroup => 'document')];
}

/** The system drag this window started: what it carried, and whether it came back to a drop here. */
interface OwnDrag {
	source: FileDragSource;
	uris: string[];
	id: number;
	/** A drop landed in this window, which acts on it itself: the end of the drag says nothing more. */
	droppedHere: boolean;
	/** The end has been reported (the plugin sends it as an event and, on Windows, with the command's answer too). */
	finished: boolean;
}

/**
 * The file drag over `dragSession`. A press on a row of a listing becomes a drag after 4 px; the
 * selection is what is dragged (an unselected row is selected first, and a Ctrl or Shift press on
 * one is a selection gesture, not a drag). Every move hit-tests the marked elements under the
 * pointer (`dropTargets.ts`) and says what a release would do; the modifier keys are read live, so
 * the pill follows them; the planner is asked in the background which volume the target is on. A
 * target that is a folder, tab or sidebar item springs open after the setting's delay and springs
 * back when the pointer leaves it, unless dropped; Esc cancels everything.
 */
export function createFileDrag(deps: FileDragDeps): FileDrag {
	const clock = deps.clock ?? realClock;
	const session = createDragSession<FileDragSource, FileDropTarget>({
		startThresholdPx: FILE_DRAG_START_PX,
		announce: deps.announce,
		clock: deps.clock,
		reducedMotion: deps.reducedMotion,
		root: deps.root,
		sameTarget: sameFileTarget,
	});

	// What one press and its drag keep; reset when it ends.
	let modifiers: DropModifiers = NO_MODIFIERS;
	let controlRef: DragControl<FileDragSource, FileDropTarget> | null = null;
	let sourceRef: FileDragSource | null = null;
	let lastPoint: Point = { x: 0, y: 0 };
	let lastSpot: DropSpot | null = null;
	let markedElement: HTMLElement | null = null;
	let lastKey = '';
	let lastPlanKey = '';
	let release: (() => void) | null = null;
	let springs: Spring[] = [];
	/** Counts the drags that have ended, so a spring still opening can tell its drag is over. */
	let generation = 0;
	let springLock: Point | null = null;
	let scroller: HTMLElement | null = null;
	let scrollTimer: unknown = null;
	let cleanup: (() => void) | null = null;
	let pendingMenu: (() => void) | null = null;
	let pressRight = false;
	const locations = new Map<string, Location>();
	const resolving = new Map<string, Promise<Location>>();
	const facts = new Map<string, PlanFact>();
	const planning = new Set<string>();
	let pressed: FileDragPress | null = null;
	// Leaving the window: the hand-over to the system, and the system drag this window started.
	let leftDocument = false;
	let handing = false;
	let handedOver = false;
	let handOffFailed = false;
	let prefetch: Promise<Location[]> | null = null;
	let own: OwnDrag | null = null;
	let ownTimer: unknown = null;
	let pendingEnded: DragEnded | null = null;

	const rootElement = () =>
		deps.root ? deps.root() : typeof document === 'undefined' ? null : document.documentElement;

	const resolveFolder = (ref: string, handle: ListingHandle, entry: number): Promise<Location> => {
		const known = locations.get(ref);
		if (known) return Promise.resolve(known);
		let pending = resolving.get(ref);
		if (!pending) {
			pending = deps.entryLocation(handle, entry).then((location) => {
				locations.set(ref, location);
				return location;
			});
			resolving.set(ref, pending);
			pending.then(
				() => refresh(),
				() => {},
			);
		}
		return pending;
	};

	/** The folder a spot names, as far as it is known now. */
	const locationOf = (spot: DropSpot): { location: Location | null; readOnly: boolean } => {
		switch (spot.kind) {
			case 'folder': {
				const ref = parseFolderRef(spot.ref);
				if (ref) void resolveFolder(spot.ref, ref.handle, ref.entry).catch(() => {});
				return { location: locations.get(spot.ref) ?? null, readOnly: false };
			}
			case 'pane': {
				const pane = deps.pane(Number(spot.ref));
				return { location: pane?.location ?? null, readOnly: pane?.readOnly ?? false };
			}
			case 'place':
			case 'crumb':
				return { location: { display: spot.label, uri: spot.ref }, readOnly: false };
			case 'tab':
				return { location: deps.tabLocation(Number(spot.ref)), readOnly: false };
			default:
				return { location: null, readOnly: false };
		}
	};

	const canLink = (source: FileDragSource, location: Location | null) =>
		canLinkSource(source) && (location ? location.uri.startsWith('file:') : true);

	const planKey = (source: FileDragSource, location: Location) =>
		`${source.handle ?? 'external'}|${normaliseUri(location.uri)}`;

	/** Copies, moves or links the dragged files into `destination`, by selection or, for files from outside, by location. */
	const transferFiles = async (
		source: FileDragSource,
		kind: 'copy' | 'move' | 'link',
		destination: Location,
	): Promise<void> => {
		if (source.external) {
			await deps.transferLocations?.(kind, source.external.locations, destination);
		} else if (source.session) {
			await deps.transfer(kind, source.session, destination);
		}
	};

	const askPlanner = (source: FileDragSource, location: Location) => {
		const key = planKey(source, location);
		const label = deps.windowLabel();
		if (facts.has(key) || planning.has(key) || label === null) return;
		planning.add(key);
		const request: JobRequest = {
			kind: { kind: 'copy' },
			sources: source.external
				? { kind: 'locations', locations: source.external.locations }
				: { kind: 'selection', handle: source.handle!, spec: source.spec! },
			destination: location,
			name: null,
			options: NO_OPTIONS,
			originWindow: label,
		};
		deps
			.plan(request)
			.then(
				(preview) => {
					facts.set(key, { volume: preview.sameVolume ? 'same' : 'different', error: null });
				},
				(error: unknown) => {
					facts.set(key, { volume: 'unknown', error: commandFailure(error) });
				},
			)
			.finally(() => {
				planning.delete(key);
				refresh();
			});
	};

	// --- Springs -----------------------------------------------------------------------------

	const revertSprings = () => {
		const all = springs;
		springs = [];
		for (const spring of all.reverse()) void spring.revert().catch(() => {});
		if (all.length > 0) deps.announce(t('dnd.announce.sprungBack'));
	};

	const settleSprings = (spot: DropSpot | null) => {
		if (!spot) return;
		let reverted = false;
		while (springs.length > 0 && !springs[springs.length - 1]!.stays(spot)) {
			void springs
				.pop()!
				.revert()
				.catch(() => {});
			reverted = true;
		}
		if (reverted) deps.announce(t('dnd.announce.sprungBack'));
	};

	const fireSpring = async (spot: DropSpot, target: FileDropTarget) => {
		if (!sourceRef) return;
		const mine = generation;
		const ended = () => mine !== generation;
		if (markedElement) {
			markedElement.removeAttribute(SPRING_ATTRIBUTE);
			markedElement.style.removeProperty('--wp-drop-spring-ms');
		}
		springLock = lastPoint;
		try {
			if (spot.kind === 'tab') {
				const id = Number(spot.ref);
				const previous = deps.activeTab();
				if (previous === id) return;
				await deps.activate(id);
				if (ended()) {
					// Released while the tab was opening: put back what was shown.
					if (previous !== null) await deps.activate(previous);
					return;
				}
				springs.push({
					stays: (next) => next.inStrip || next.pane !== null,
					revert: () => (previous === null ? Promise.resolve() : deps.activate(previous)),
				});
				deps.announce(tf('dnd.announce.sprungTab', { target: target.label }));
				return;
			}
			let location = target.location;
			if (!location && spot.kind === 'folder') {
				const ref = parseFolderRef(spot.ref);
				if (ref) location = await resolveFolder(spot.ref, ref.handle, ref.entry);
				if (ended()) return;
			}
			const pane = spot.kind === 'folder' ? spot.pane : deps.activeTab();
			if (!location || pane === null) return;
			const shown = deps.pane(pane)?.location;
			if (shown && normaliseUri(shown.uri) === normaliseUri(location.uri)) return;
			await deps.navigate(pane, location);
			if (ended()) {
				// Released while the folder was opening: go back at once.
				await deps.back(pane);
				return;
			}
			springs.push({ stays: (next) => next.pane === pane, revert: () => deps.back(pane) });
			deps.announce(tf('dnd.announce.sprungFolder', { target: target.label }));
		} catch (error) {
			console.warn('spring-load failed', error);
		} finally {
			// The view is new: look again, and let another spring open once the pointer has moved.
			lastKey = '';
			clock.setTimeout(() => refresh(), 80);
		}
	};

	const armSpring = (
		control: DragControl<FileDragSource, FileDropTarget>,
		spot: DropSpot,
		target: FileDropTarget,
	) => {
		if (!SPRING_KINDS.has(spot.kind) || target.blocked) return;
		if (
			springLock &&
			Math.hypot(lastPoint.x - springLock.x, lastPoint.y - springLock.y) <= SPRING_JITTER_PX
		) {
			return;
		}
		springLock = null;
		const ms = deps.springMs();
		spot.element.style.setProperty('--wp-drop-spring-ms', `${ms}ms`);
		spot.element.setAttribute(SPRING_ATTRIBUTE, '');
		control.hold('spring', ms, () => void fireSpring(spot, target));
	};

	// --- The target under the pointer --------------------------------------------------------

	const apply = (control: DragControl<FileDragSource, FileDropTarget>, spot: DropSpot | null) => {
		const source = control.source;
		lastSpot = spot;
		// Once the pointer has moved off the place a spring opened, a target may arm another.
		if (
			springLock &&
			Math.hypot(lastPoint.x - springLock.x, lastPoint.y - springLock.y) > SPRING_JITTER_PX
		) {
			springLock = null;
			lastKey = '';
		}
		if (!spot) {
			markOver(markedElement, null, null);
			markedElement = null;
			lastKey = '';
			lastPlanKey = '';
			control.clearHold('spring');
			control.clearHold('plan');
			control.setTarget(null);
			show(control, pillFor(source, null));
			return;
		}
		settleSprings(spot);
		const { location, readOnly } = locationOf(spot);
		const ref = spot.kind === 'folder' ? parseFolderRef(spot.ref) : null;
		const selfRow = source.external
			? spot.kind === 'folder' &&
				location !== null &&
				source.external.locations.some(
					(file) => normaliseUri(file.uri) === normaliseUri(location.uri),
				)
			: ref !== null &&
				source.session !== null &&
				ref.handle === source.handle &&
				isSelected(source.session.store.getState().selection, ref.entry);
		const here = isSourceFolder(source, location);
		// The pane the files are in, showing the folder they are in: nothing to drop on.
		if (spot.kind === 'pane' && here) {
			markOver(markedElement, null, null);
			markedElement = null;
			lastKey = '';
			lastPlanKey = '';
			control.clearHold('spring');
			control.clearHold('plan');
			control.setTarget(null);
			show(control, pillFor(source, null));
			return;
		}
		const target = evaluateTarget({
			source,
			spot,
			location,
			readOnly,
			selfRow,
			plan: location && !here ? (facts.get(planKey(source, location)) ?? null) : null,
			modifiers,
			rule: deps.rule(),
			canLink: canLink(source, location),
			trashAvailable: deps.trashAvailable(),
		});
		control.setTarget(target);
		show(control, pillFor(source, target, modifiers.alt));
		const previous = markedElement;
		markedElement = spot.element;
		markOver(previous, spot.element, target.blocked ? 'blocked' : 'ok');

		// A spring and a question to the planner are each started once for a target; the planner's waits
		// for the target's folder, which a folder row has to ask Rust for.
		const key = `${spot.kind}:${spot.ref}`;
		if (key !== lastKey) {
			lastKey = key;
			control.clearHold('spring');
			armSpring(control, spot, target);
		}
		const planned = location && !here && !target.blocked ? `${key}|${location.uri}` : '';
		if (planned !== lastPlanKey) {
			lastPlanKey = planned;
			control.clearHold('plan');
			if (location && planned) {
				control.hold('plan', PLAN_REST_MS, () => askPlanner(source, location));
			}
		}
	};

	/** Shows the pill and tells the root what a release would do, so the cursor follows. */
	const show = (control: DragControl<FileDragSource, FileDropTarget>, pill: DragPill) => {
		control.setPill(pill);
		rootElement()?.setAttribute(FILE_DRAG_ATTRIBUTE, pill.kind);
	};

	const hit = (point: Point) => dropSpotAt(point, deps.hit);

	function refresh() {
		const control = controlRef;
		if (!control || session.store.getState().phase !== 'dragging') return;
		apply(control, lastSpot ? hit(lastPoint) : null);
	}

	// --- Scrolling at the edge ---------------------------------------------------------------

	const stopScrolling = () => {
		if (scrollTimer !== null) clock.clearTimeout(scrollTimer);
		scrollTimer = null;
	};

	const tickScroll = () => {
		scrollTimer = null;
		const element = scroller;
		if (!element || session.store.getState().phase !== 'dragging') return;
		const rect = element.getBoundingClientRect();
		const step = edgeScrollStep(rect.top, rect.bottom, lastPoint.y);
		if (step === 0) return;
		element.scrollTop += step;
		refresh();
		scrollTimer = clock.setTimeout(tickScroll, SCROLL_TICK_MS);
	};

	const followEdge = (element: HTMLElement | null, point: Point) => {
		scroller = element;
		if (!element) return stopScrolling();
		const rect = element.getBoundingClientRect();
		if (edgeScrollStep(rect.top, rect.bottom, point.y) === 0) return stopScrolling();
		if (scrollTimer === null) scrollTimer = clock.setTimeout(tickScroll, SCROLL_TICK_MS);
	};

	// --- Ending ------------------------------------------------------------------------------

	const reset = () => {
		cleanup?.();
		cleanup = null;
		stopScrolling();
		if (markedElement) clearMark(markedElement);
		markedElement = null;
		rootElement()?.removeAttribute(FILE_DRAG_ATTRIBUTE);
		controlRef = null;
		sourceRef = null;
		generation += 1;
		springs = [];
		lastSpot = null;
		lastKey = '';
		lastPlanKey = '';
		springLock = null;
		scroller = null;
		locations.clear();
		resolving.clear();
		facts.clear();
		planning.clear();
		modifiers = NO_MODIFIERS;
		pressed = null;
		pressRight = false;
		leftDocument = false;
		handing = false;
		handOffFailed = false;
		prefetch = null;
	};

	const releaseSource = () => {
		const done = release;
		release = null;
		done?.();
	};

	/** A press that ended without becoming a drag: a click, as far as everyone else is concerned. */
	const endClick = () => {
		const menu = pendingMenu;
		pendingMenu = null;
		reset();
		menu?.();
	};

	// --- The drop ----------------------------------------------------------------------------

	const dropHere = async (
		control: DragControl<FileDragSource, FileDropTarget>,
		spot: DropSpot | null,
		point: Point,
		held: (() => void) | null,
	) => {
		const source = control.source;
		const target = control.target();
		const mods = modifiers;
		// Only what the drag still holds is released, and only once: the picker may take it over.
		let done = () => held?.();
		const keep = spot !== null && springs.length > 0 && springs[springs.length - 1]!.stays(spot);
		if (!keep) revertSprings();
		else springs = [];
		if (!spot || !target) {
			deps.announce(t('dnd.announce.nothing'));
			return done();
		}
		if (target.blocked || target.outcome === null) {
			const reason = target.blocked ? blockedText(target.blocked, target) : '';
			deps.say(reason.charAt(0).toUpperCase() + reason.slice(1));
			return done();
		}
		try {
			switch (target.outcome) {
				case 'trash':
					if (source.session) await deps.moveToTrash(source.session);
					return;
				case 'open':
					await deps.openFolders({
						source,
						target: target.kind === 'chip' ? 'chip' : 'plus',
						group: target.kind === 'chip' ? Number(target.ref) : null,
						split: mods.alt,
					});
					return;
				case 'copy':
				case 'move':
				case 'link': {
					const location = await destinationOf(spot, target);
					if (location) await transferFiles(source, target.outcome, location);
					return;
				}
				case 'ask': {
					const location = await destinationOf(spot, target);
					if (!location) return;
					const verbs = pickerVerbs({
						canMove: !source.readOnly && !isSourceFolder(source, location),
						canLink: canLink(source, location),
					});
					const handOver = done;
					done = () => {};
					deps.announce(
						tf('dnd.announce.picker', { what: subjectText(source), target: target.label }),
					);
					deps.openPicker({
						position: point,
						what: subjectText(source),
						target: { ...target, location },
						verbs,
						choose: (verb) => {
							void transferFiles(source, verb, location).finally(handOver);
						},
						cancel: () => {
							deps.announce(t('dnd.announce.pickerCancelled'));
							handOver();
						},
					});
					return;
				}
			}
		} catch (error) {
			console.warn('file drop failed', error);
		} finally {
			done();
		}
	};

	/** Where the files would go: a folder row's location is asked for if it has not come yet. */
	const destinationOf = async (
		spot: DropSpot,
		target: FileDropTarget,
	): Promise<Location | null> => {
		if (target.location) return target.location;
		const ref = spot.kind === 'folder' ? parseFolderRef(spot.ref) : null;
		return ref ? resolveFolder(spot.ref, ref.handle, ref.entry) : null;
	};

	// --- Leaving the window ------------------------------------------------------------------

	const dragging = (control: DragControl<FileDragSource, FileDropTarget>) =>
		controlRef === control && session.store.getState().phase === 'dragging';

	/** Resolves the dragged items' locations ahead of time, for a selection small enough to be cheap. */
	const prefetchLocations = (source: FileDragSource) => {
		const out = deps.outbound;
		if (!out?.available() || !source.session || source.handle === null || !source.spec) return;
		if (source.count > PREFETCH_LIMIT) return;
		prefetch = out.resolve(source.handle, source.spec);
		// A failure is reported when the drag leaves the window and asks again.
		prefetch.catch(() => {});
	};

	const wantsToLeave = (source: FileDragSource, point: Point): boolean => {
		const out = deps.outbound;
		if (!out || !out.available() || handing || handOffFailed) return false;
		// Only a drag of a listing's selection with the primary button can become a system drag.
		if (source.external || source.rightButton) return false;
		return leftDocument || out.outside(point);
	};

	const forgetOwn = () => {
		if (ownTimer !== null) clock.clearTimeout(ownTimer);
		ownTimer = null;
		own = null;
	};

	/** What the end of this window's system drag says, once; the drop that came back here says its own. */
	const finishOutbound = (ended: DragEnded) => {
		const record = own;
		if (!record || record.finished || ended.id !== record.id) return;
		record.finished = true;
		const what = subjectText(record.source);
		if (!record.droppedHere) {
			switch (ended.outcome) {
				case 'dropped-copy':
					deps.announce(tf('dnd.out.copied', { what }));
					break;
				// Nothing is deleted or assumed gone here: whoever moved the files (or asked for them to
				// be moved) accounts for the originals, and the listing follows its watcher.
				case 'dropped-move':
					deps.announce(tf('dnd.out.moved', { what }));
					break;
				case 'dropped-link':
					deps.announce(tf('dnd.out.linked', { what }));
					break;
				case 'cancelled':
					deps.announce(t('drag.announce.cancelled'));
					break;
				case 'failed':
					deps.say(tf('dnd.out.failed', { reason: ended.reason ?? what }));
					break;
			}
		}
		// A drop can still arrive after the end (Windows delivers it once the drag has returned).
		if (ownTimer !== null) clock.clearTimeout(ownTimer);
		ownTimer = clock.setTimeout(forgetOwn, OWN_DRAG_MS);
	};

	/**
	 * The pointer has left the window with files in hand: the drag continues as the system's. The
	 * locations come from Rust (a prefetch when the selection is small), the plugin starts the drag
	 * with the allowed actions, and on success the in-page drag ends quietly. A drag that cannot be
	 * handed over stays in the page, says so, and tries again only after the pointer has been back inside.
	 */
	const handOff = async (control: DragControl<FileDragSource, FileDropTarget>) => {
		const out = deps.outbound;
		const source = control.source;
		if (!out || handing || !source.session || source.handle === null || !source.spec) return;
		handing = true;
		// Nothing in the window is a target any more.
		apply(control, null);
		control.setPill(null);
		const what = subjectText(source);
		const fail = (text: string) => {
			handing = false;
			handOffFailed = true;
			deps.say(text);
			// The drag carries on in the page, so it shows what is dragged again.
			if (dragging(control)) apply(control, null);
		};
		let locations: Location[];
		try {
			locations = await (prefetch ?? out.resolve(source.handle, source.spec));
		} catch (error) {
			console.warn('could not resolve the dragged items', error);
			return fail(tf('dnd.out.refused', { what }));
		}
		// The drag ended while the locations were coming: there is nothing to hand over.
		if (!dragging(control)) {
			handing = false;
			return;
		}
		const uris = locations.map((location) => location.uri);
		if (uris.length === 0 || !uris.every((uri) => uri.startsWith('file:'))) {
			return fail(tf('dnd.out.unsupported', { what: capitalise(what) }));
		}
		const actions: DragAction[] = [
			'copy',
			...(source.readOnly ? [] : (['move'] as const)),
			...(canLinkSource(source) ? (['link'] as const) : []),
		];
		let started: OutboundStarted;
		try {
			started = await out.start({ uris, actions });
		} catch (error) {
			const kind = nativeDndErrorKind(error);
			console.warn('the system drag did not start', error);
			return fail(
				kind === 'buttonNotPressed' || kind === 'alreadyActive'
					? tf('dnd.out.refused', { what })
					: tf('dnd.out.failed', {
							reason: (error as { message?: string } | null)?.message ?? what,
						}),
			);
		}
		forgetOwn();
		own = { source, uris, id: started.id, droppedHere: false, finished: false };
		// If the end never arrives, a later drop of the same files from elsewhere must not pass for this one.
		ownTimer = clock.setTimeout(forgetOwn, OWN_DRAG_MAX_MS);
		handing = false;
		// The system has the pointer now: the in-page drag is over, and says where the files went.
		if (dragging(control)) {
			handedOver = true;
			session.cancel();
			handedOver = false;
		}
		const ended = started.ended ?? (pendingEnded?.id === started.id ? pendingEnded : null);
		pendingEnded = null;
		if (ended) finishOutbound(ended);
	};

	const handlers: DragHandlers<FileDragSource, FileDropTarget> = {
		start(control, point) {
			const current = pressed;
			// A drag has a press behind it, or it is files the system is dragging in.
			if (!current && !control.source.external) return;
			controlRef = control;
			sourceRef = control.source;
			lastPoint = point;
			pendingMenu = null;
			if (current) {
				const { store } = current.session;
				if (!isSelected(store.getState().selection, current.entry.id)) {
					store.getState().click(current.position, current.entry.id);
				}
				if (current.tab !== null) release = deps.retain(current.tab);
			}
			rootElement()?.setAttribute(FILE_DRAG_ATTRIBUTE, 'idle');
			control.announce(dragText(control.source));
			show(control, pillFor(control.source, null));
			if (current) prefetchLocations(control.source);
		},
		move(control, point) {
			controlRef = control;
			lastPoint = point;
			if (!deps.outbound?.outside(point)) {
				leftDocument = false;
				if (!control.source.external) handOffFailed = false;
			}
			if (wantsToLeave(control.source, point)) {
				void handOff(control);
				return;
			}
			// While the hand-over is under way nothing in the window is a target.
			if (handing) return;
			const spot = hit(point);
			followEdge(spot?.scroller ?? null, point);
			apply(control, spot);
		},
		drop(control, point) {
			// What a release does is read once more with the keys as they are now.
			const spot = lastSpot ? hit(point) : null;
			apply(control, spot);
			const held = release;
			release = null;
			const outcome = dropHere(control, spot, point, held);
			reset();
			return outcome;
		},
		cancel(control, reason) {
			revertSprings();
			if (handedOver) {
				control.announce(tf('dnd.out.started', { what: subjectText(control.source) }));
			} else if (reason === 'left') {
				control.announce(t('dnd.announce.left'));
			} else {
				control.announce(t('drag.announce.cancelled'));
			}
			releaseSource();
			reset();
		},
	};

	const drag: FileDrag = {
		session,
		press(input) {
			if (!deps.windowLabel()) return false;
			const { model, store } = input.session;
			// Nothing is dragged out of the Trash here: Restore is its own command.
			if (model.layout === 'trash') return false;
			const state = store.getState();
			const selected = isSelected(state.selection, input.entry.id);
			// Ctrl or Shift on a row that is not selected is a selection gesture; it must stay one.
			if (!selected && (input.modifiers.ctrl || input.modifiers.shift)) return false;
			const selection = selected ? state.selection : selectOnly(input.entry.id);
			const count = selectedCount(selection, model.count);
			if (count === 0) return false;
			const source: FileDragSource = {
				session: input.session,
				tab: input.tab,
				handle: model.handle,
				spec: selectionSpec(selection),
				count,
				name: count === 1 ? input.entry.name : null,
				groups: stackGroups(input.entry.group, count),
				folder: model.location,
				readOnly: model.readOnly,
				rightButton: input.button === 2,
			};
			pressed = input;
			modifiers = input.modifiers;
			pressRight = input.button === 2;
			const track = (event: KeyboardEvent | PointerEvent) => {
				modifiers = modifiersOf(event);
				if (event.type === 'keydown' || event.type === 'keyup') {
					if (
						(event as KeyboardEvent).key === 'Alt' &&
						session.store.getState().phase === 'dragging'
					) {
						// Alt alone would move focus to the window's menu; here it opens the picker.
						event.preventDefault();
					}
					refresh();
				}
			};
			const upOrCancel = () => {
				// Ended before the threshold: it was a click. After it, the session's own handlers finish the drag.
				if (session.store.getState().phase === 'pending') endClick();
			};
			const noSelect = (event: Event) => event.preventDefault();
			window.addEventListener('pointermove', track, true);
			window.addEventListener('keydown', track, true);
			window.addEventListener('keyup', track, true);
			window.addEventListener('pointerup', track, true);
			window.addEventListener('pointerup', upOrCancel, true);
			window.addEventListener('pointercancel', upOrCancel, true);
			document.addEventListener('selectstart', noSelect, true);
			// Where the pointer is not captured, the document is what sees it leave the window.
			const root = rootElement();
			const onDocumentLeave = () => drag.leftWindow();
			root?.addEventListener('pointerleave', onDocumentLeave);
			cleanup = () => {
				root?.removeEventListener('pointerleave', onDocumentLeave);
				window.removeEventListener('pointermove', track, true);
				window.removeEventListener('keydown', track, true);
				window.removeEventListener('keyup', track, true);
				window.removeEventListener('pointerup', track, true);
				window.removeEventListener('pointerup', upOrCancel, true);
				window.removeEventListener('pointercancel', upOrCancel, true);
				document.removeEventListener('selectstart', noSelect, true);
			};
			const started = session.begin(
				{
					pointerId: input.pointerId,
					clientX: input.clientX,
					clientY: input.clientY,
					element: input.element,
					source,
				},
				handlers,
			);
			if (!started) {
				cleanup();
				cleanup = null;
				pressed = null;
				pressRight = false;
			}
			return started;
		},
		deferMenu(open) {
			if (!pressRight || session.store.getState().phase !== 'pending') return false;
			pendingMenu = open;
			return true;
		},
		beginNative(input) {
			if (!deps.windowLabel() || input.files.length === 0) return null;
			const uris = input.files.map((file) => file.uri);
			const returning = own !== null && sameUris(own.uris, uris) ? own : null;
			// This window's own drag coming back keeps what it was (the folder, whether it could be
			// moved); anything else is files from another application or window.
			const source: FileDragSource = returning
				? {
						...returning.source,
						session: null,
						handle: null,
						spec: null,
						tab: null,
						rightButton: false,
						external: { locations: [...input.files], own: true },
					}
				: externalSource(input.files);
			modifiers = input.modifiers;
			const external = session.beginExternal({ point: input.point, source }, handlers);
			if (!external) return null;
			return {
				move(point, keys) {
					modifiers = keys;
					external.move(point);
				},
				drop(point, keys, selfDrop) {
					modifiers = keys;
					if (returning && (selfDrop || own === returning)) returning.droppedHere = true;
					return external.drop(point);
				},
				leave: () => external.cancel('left'),
			};
		},
		dragEnded(event) {
			if (own) finishOutbound(event);
			else if (handing) pendingEnded = event;
		},
		leftWindow() {
			leftDocument = true;
			const control = controlRef;
			if (control && dragging(control) && wantsToLeave(control.source, lastPoint)) {
				void handOff(control);
			}
		},
		dispose() {
			session.dispose();
			reset();
			forgetOwn();
		},
	};
	return drag;
}

/** A sentence's first letter in capitals. */
function capitalise(text: string): string {
	return text.charAt(0).toUpperCase() + text.slice(1);
}
