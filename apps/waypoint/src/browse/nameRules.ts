// A mirror of Rust's `validate_name`, so the rename field can say what is wrong while the person types
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/**
 * How names compare where they will live. `sensitive` is the POSIX rule (Linux); `insensitive` is
 * Windows', which also refuses reserved device names, `<>:"|?*`, control characters and a trailing
 * dot or space. Rust's `CaseRule` makes the same choice.
 */
export type NameRule = 'sensitive' | 'insensitive';

/** What is wrong with a name. Each problem has its own sentence in the catalogue. */
export type NameProblem =
	| { kind: 'empty' }
	| { kind: 'dots' }
	| { kind: 'nul' }
	| { kind: 'slash' }
	| { kind: 'backslash' }
	| { kind: 'tooLong'; limit: number }
	| { kind: 'forbidden'; character: string }
	| { kind: 'trailing' }
	| { kind: 'reserved' };

const MAX_NAME = 255;
const WINDOWS_FORBIDDEN = '<>:"|?*';
// `char::is_control`: the C0 and C1 control characters.
const CONTROL = /[\u0000-\u001f\u007f-\u009f]/u;

/** The rule for local files on the host this runs on: Windows names compare without case. */
export function hostNameRule(): NameRule {
	try {
		return /windows/i.test(globalThis.navigator?.userAgent ?? '') ? 'insensitive' : 'sensitive';
	} catch {
		return 'sensitive';
	}
}

/** Whether `name` is a device name Windows reserves in every folder (`CON`, `NUL`, `COM1`…), even with an extension. */
export function isReservedName(name: string): boolean {
	const stem = (name.split('.')[0] ?? '').replace(/ +$/u, '').toUpperCase();
	if (['CON', 'PRN', 'AUX', 'NUL'].includes(stem)) return true;
	return /^(?:COM|LPT)[1-9]$/u.test(stem);
}

/**
 * The first reason `name` cannot be the name of an entry under `rule`, or `null` when it can.
 * Rust still decides (a provider may refuse more); this exists for instant feedback and is tested
 * against the same table as `validate_name`.
 */
export function checkName(name: string, rule: NameRule): NameProblem | null {
	if (name === '') return { kind: 'empty' };
	if (name === '.' || name === '..') return { kind: 'dots' };
	if (name.includes('\u0000')) return { kind: 'nul' };
	if (name.includes('/')) return { kind: 'slash' };
	if (rule === 'sensitive') {
		if (new TextEncoder().encode(name).length > MAX_NAME) {
			return { kind: 'tooLong', limit: MAX_NAME };
		}
		return null;
	}
	// UTF-16 units, which is what `String.length` counts.
	if (name.length > MAX_NAME) return { kind: 'tooLong', limit: MAX_NAME };
	if (name.includes('\\')) return { kind: 'backslash' };
	for (const character of name) {
		if (CONTROL.test(character) || WINDOWS_FORBIDDEN.includes(character)) {
			return { kind: 'forbidden', character };
		}
	}
	if (/[. ]$/u.test(name)) return { kind: 'trailing' };
	if (isReservedName(name)) return { kind: 'reserved' };
	return null;
}
