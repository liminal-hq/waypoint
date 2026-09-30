// Forwards `console.*` into `@tauri-apps/plugin-log` so webview messages join the native log
// stream (see `src-tauri/src/lib.rs`), prefixed with the window label since several webviews
// feed the one stream. `console.log` maps to `info`; the plugin has no separate "log" level.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { isTauri } from '@tauri-apps/api/core';
import { debug, error, info, warn } from '@tauri-apps/plugin-log';

type ConsoleMethod = 'log' | 'debug' | 'info' | 'warn' | 'error';
type CallSite = { file: string; line: number };
type PluginLogger = (message: string, options?: { file?: string; line?: number }) => Promise<void>;

function serialiseConsoleArg(arg: unknown): string {
	if (typeof arg === 'string') return arg;
	if (arg instanceof Error) return arg.stack ?? `${arg.name}: ${arg.message}`;
	try {
		return JSON.stringify(arg);
	} catch {
		return serialiseCircular(arg);
	}
}

/**
 * Fallback for values `JSON.stringify` can't handle directly (circular
 * references, mainly). Retried once with a replacer that tags repeated
 * object references instead of throwing, so we keep the object's shape
 * instead of degrading straight to `String(arg)` -> the useless
 * `"[object Object]"`.
 */
function serialiseCircular(arg: unknown): string {
	const seen = new WeakSet<object>();
	try {
		return JSON.stringify(arg, (_key, value: unknown) => {
			if (typeof value === 'object' && value !== null) {
				if (seen.has(value)) return '[Circular]';
				seen.add(value);
			}
			return value;
		});
	} catch {
		// Something JSON.stringify still can't touch even with a replacer (e.g.
		// a BigInt somewhere in the structure) — fall back to a type-tagged
		// string so at least the value's kind survives, rather than a bare
		// "[object Object]".
		const ctorName = (arg as { constructor?: { name?: string } } | null)?.constructor?.name;
		return `[unserialisable ${ctorName ?? typeof arg}]`;
	}
}

/**
 * Parses a single V8 (`at name (file:line:col)` / `at file:line:col`) stack
 * frame line.
 */
function parseV8Frame(frame: string): CallSite | undefined {
	const withFn = /^at\s+.*?\s+\((.+?):(\d+):(\d+)\)$/.exec(frame);
	const bare = withFn ? undefined : /^at\s+(.+?):(\d+):(\d+)$/.exec(frame);
	const match = withFn ?? bare;
	if (!match) return undefined;
	const [, file, line] = match;
	return file ? { file, line: Number(line) } : undefined;
}

/** Parses a single WebKit/JavaScriptCore (`name@file:line:col`) stack frame line. */
function parseJscFrame(frame: string): CallSite | undefined {
	const at = frame.lastIndexOf('@');
	const location = at === -1 ? frame : frame.slice(at + 1);
	const match = /^(.+?):(\d+):(\d+)$/.exec(location);
	if (!match) return undefined;
	const [, file, line] = match;
	return file ? { file, line: Number(line) } : undefined;
}

/**
 * Extracts the real application call site from a stack captured with
 * `new Error().stack` *directly inside* `forwardConsole`'s wrapper (no
 * extra indirection in between). Stack trace formats vary by engine, so
 * this is deliberately conservative: if it can't confidently identify the
 * caller's frame, it returns `undefined` rather than guessing.
 *
 * Frame layout for a stack captured this way:
 *   - V8 (Chromium/Node): line 0 is the `Error` header; line 1 is this
 *     wrapper's own frame (where `new Error()` was constructed); line 2 is
 *     whoever called `console.*` — the frame we want.
 *   - JavaScriptCore/WebKit (no header line): line 0 is the wrapper's own
 *     frame; line 1 is the caller.
 */
function extractCallSite(stack: string | undefined): CallSite | undefined {
	if (!stack) return undefined;
	const lines = stack.split('\n').map((line) => line.trim());
	const isV8 = lines[0]?.startsWith('Error');
	const frame = isV8 ? lines[2] : lines[1];
	if (!frame) return undefined;
	return isV8 ? parseV8Frame(frame) : parseJscFrame(frame);
}

function forwardConsole(method: ConsoleMethod, logger: PluginLogger, prefix: string): void {
	const original = console[method].bind(console);
	console[method] = (...args: unknown[]) => {
		original(...args);
		const callSite = extractCallSite(new Error().stack);
		const message = prefix + args.map(serialiseConsoleArg).join(' ');
		// Forwarding into the native log stream is best-effort: whether the
		// bridge is entirely absent (see `initLogger`'s `isTauri()` guard) or
		// present but failing for some other reason (misconfigured plugin,
		// IPC error, ...), it must never surface as an unhandled rejection and
		// break app code that just called console.log/warn/etc.
		void logger(message, callSite).catch(() => {});
	};
}

let installed = false;

/** Call once at startup with this webview's window label — see `main.tsx`.
 * No-op outside a real Tauri webview (plain `bun run dev`, vitest, ...) and
 * safe to call more than once. */
export function initLogger(windowLabel: string): void {
	if (installed) return;
	if (!isTauri()) return;
	installed = true;
	const prefix = `[${windowLabel}] `;
	forwardConsole('log', info, prefix);
	forwardConsole('debug', debug, prefix);
	forwardConsole('info', info, prefix);
	forwardConsole('warn', warn, prefix);
	forwardConsole('error', error, prefix);
}
