// The default action of a file drop and what the modifier keys and the pointer button change: a pure rule
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { DropActionRule } from '@liminal-hq/waypoint-protocol/generated/DropActionRule';

/** What a drop can do with the dragged files; `ask` opens the action picker instead of choosing. */
export type DropVerb = 'copy' | 'move' | 'link' | 'ask';

/** The modifier keys, read when the pointer is released and again whenever one changes. */
export interface DropModifiers {
	ctrl: boolean;
	shift: boolean;
	alt: boolean;
}

export const NO_MODIFIERS: DropModifiers = { ctrl: false, shift: false, alt: false };

/** Whether the sources and the target are on one volume, as the planner says; `unknown` until it has answered. */
export type VolumeRelation = 'same' | 'different' | 'unknown';

export interface DropActionInput {
	rule: DropActionRule;
	volume: VolumeRelation;
	modifiers: DropModifiers;
	/** The drag was made with the right button, which opens the picker on release. */
	rightButton: boolean;
	/** The sources can be moved: their folder can be written to. */
	canMove: boolean;
	/** The engine can link them here (both ends are local). */
	canLink: boolean;
	/**
	 * The files come from another application, which did not say what it allows (a drag in carries
	 * no promise of allowed actions, and on Wayland no modifier keys, only the action the compositor negotiated), so a copy-only source (an
	 * archive manager, a mail attachment, a browser's temporary file) cannot be told from the rest.
	 * With no modifier they are copied under the by-volume rule, never moved.
	 */
	foreign?: boolean;
}

export interface DropChoice {
	verb: DropVerb;
	/** The verb is a guess made while the planner has not said which volume the target is on. */
	pending: boolean;
	/** A modifier or the right button chose it, not the rule. */
	forced: boolean;
}

/**
 * The action a release would take (SPEC §6, `docs/interactions.md` §3.1 and §3.2):
 * - the right button or Alt opens the picker, whatever the rule says;
 * - Ctrl copies, Shift moves, Ctrl+Shift links (where the engine can; otherwise it copies, the
 *   safe half of the chord), and a move asked for from a folder that cannot be written to copies;
 * - with no modifier the rule decides: always copy, always ask, or by volume (a move on one
 *   volume, a copy across; files from another application are always copied, since nothing says
 *   what their source allows). While the volume is unknown the verb is a copy and `pending` is set,
 *   so the pill can say "Move or copy" and nothing waits on the answer.
 */
export function chooseDropAction(input: DropActionInput): DropChoice {
	const { rule, volume, modifiers, rightButton, canMove, canLink, foreign = false } = input;
	const forced = (verb: DropVerb): DropChoice => ({ verb, pending: false, forced: true });
	if (rightButton || modifiers.alt) return forced('ask');
	if (modifiers.ctrl && modifiers.shift) return forced(canLink ? 'link' : 'copy');
	if (modifiers.ctrl) return forced('copy');
	if (modifiers.shift) return forced(canMove ? 'move' : 'copy');
	switch (rule) {
		case 'alwaysCopy':
			return { verb: 'copy', pending: false, forced: false };
		case 'alwaysAsk':
			return { verb: 'ask', pending: false, forced: false };
		case 'byVolume':
			if (foreign) return { verb: 'copy', pending: false, forced: false };
			if (volume === 'unknown') return { verb: 'copy', pending: true, forced: false };
			return {
				verb: volume === 'same' && canMove ? 'move' : 'copy',
				pending: false,
				forced: false,
			};
	}
}

/** What the picker can do: the verbs of a plain drop, and the two that make or open an archive (D170). */
export type PickerVerb = Exclude<DropVerb, 'ask'> | 'compress' | 'extract';

/**
 * What the picker offers, in order: Copy (Add to Archive in an archive), Move where the sources can
 * be moved, Link where they can be linked, Compress Here where an archive can be made, and Extract
 * Here where every source is an archive and can be extracted.
 */
export function pickerVerbs(options: {
	canMove: boolean;
	canLink: boolean;
	canCompress?: boolean;
	canExtract?: boolean;
}): PickerVerb[] {
	return [
		'copy',
		...(options.canMove ? (['move'] as const) : []),
		...(options.canLink ? (['link'] as const) : []),
		...(options.canCompress ? (['compress'] as const) : []),
		...(options.canExtract ? (['extract'] as const) : []),
	];
}

/** The modifier state of an event, for the one place that reads it. */
export function modifiersOf(event: {
	ctrlKey: boolean;
	shiftKey: boolean;
	altKey: boolean;
}): DropModifiers {
	return { ctrl: event.ctrlKey, shift: event.shiftKey, alt: event.altKey };
}
