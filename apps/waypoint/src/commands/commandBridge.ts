// The window's current command facts and actions, shared by everything that lists or runs commands
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useCallback, useContext, useMemo } from 'react';
import { useStore } from 'zustand';
import { createStore, type StoreApi } from 'zustand/vanilla';
import {
	emptyFacts,
	idleActions,
	type CommandActions,
	type CommandEnv,
	type CommandFacts,
} from './commandEnv';
import {
	commandDef,
	evaluateCommands,
	runCommand,
	viewOf,
	type CommandId,
	type CommandView,
} from './registry';

/**
 * Where a window keeps its command environment. The title bar's menu sits above the workspace
 * that owns the panes, the queue and the view, so the workspace publishes what the commands need
 * here and the menu, the Action bar and (later) the palette read it from one place.
 */
export interface CommandBridge {
	store: StoreApi<CommandEnv>;
	/** Merges `patch` into the facts. Nothing changes (and nobody is notified) when no value differs. */
	patchFacts(patch: Partial<CommandFacts>): void;
	patchActions(patch: Partial<CommandActions>): void;
}

export function createCommandBridge(initial?: Partial<CommandEnv>): CommandBridge {
	const store = createStore<CommandEnv>()(() => ({
		facts: emptyFacts(),
		actions: idleActions(),
		...initial,
	}));
	return {
		store,
		patchFacts(patch) {
			const { facts } = store.getState();
			const changed = (Object.keys(patch) as Array<keyof CommandFacts>).some(
				(key) => JSON.stringify(patch[key]) !== JSON.stringify(facts[key]),
			);
			if (changed) store.setState({ facts: { ...facts, ...patch } });
		},
		patchActions(patch) {
			store.setState({ actions: { ...store.getState().actions, ...patch } });
		},
	};
}

const CommandBridgeContext = createContext<CommandBridge | null>(null);

export const CommandBridgeProvider = CommandBridgeContext.Provider;

/** Shared by windows and components that have no provider, so they list nothing runnable rather than fail. */
const idleBridge = createCommandBridge();

/** The bridge a window provided, or `null` where none was (a workspace then makes its own). */
export function useProvidedCommandBridge(): CommandBridge | null {
	return useContext(CommandBridgeContext);
}

export function useCommandBridge(): CommandBridge {
	return useContext(CommandBridgeContext) ?? idleBridge;
}

/** What `useCommands` hands to a list of commands. */
export interface CommandsApi {
	/** Every command, in the registry's order, resolved against the window's state now. */
	commands: readonly CommandView[];
	/** One command by id. */
	get(id: CommandId): CommandView;
	/** Runs a command if it is offered and enabled; says whether it ran. */
	run(id: CommandId): boolean;
	facts: CommandFacts;
}

/**
 * The window's commands with their availability, recomputed when the selection, the open
 * listing, the clipboard, the history, the view or the tabs change. A palette lists `commands`
 * (filtering on `visible`, greying `enabled: false` rows with their `reason`) and runs the chosen
 * one with `run(id)`.
 */
export function useCommands(): CommandsApi {
	const bridge = useCommandBridge();
	const facts = useStore(bridge.store, (env) => env.facts);
	const commands = useMemo(() => evaluateCommands(facts), [facts]);
	const get = useCallback((id: CommandId) => viewOf(commandDef(id), facts), [facts]);
	// `run` reads the environment when called, so it is stable and a held callback never acts on stale facts.
	const run = useCallback(
		(id: CommandId) => {
			const env = bridge.store.getState();
			return runCommand(id, env.actions, env.facts);
		},
		[bridge],
	);
	return useMemo(() => ({ commands, get, run, facts }), [commands, get, run, facts]);
}
