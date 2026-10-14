// Verifies when Run as Administrator is offered and how a start, a declined prompt and a failure are handled
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import { t } from '../i18n/messages';
import { createFakeOpenWithClient } from './fakeOpenWithClient';
import {
	isRunnableEntry,
	isRunnableName,
	runAsAdministratorOffered,
	runnableFileSelected,
	runRunAsAdministratorCommand,
	startRunAsAdministrator,
} from './runAsAdministrator';
import type { ListingSession } from '../browse/useListingSession';

const selectedEntries = vi.hoisted(() => vi.fn());
vi.mock('../dnd/openFolders', () => ({ selectedEntries }));

const uris = ['file:///C:/Tools/setup.exe'];

function entry(name: string, kind: Entry['kind'] = 'file'): Entry {
	return { id: 7, name, kind, linkTarget: null } as unknown as Entry;
}

function sessionWith(
	selection: { kind: 'some'; ids: Set<number> } | { kind: 'all' } | { kind: 'none' },
	cached: Entry | undefined,
): ListingSession {
	return {
		store: { getState: () => ({ selection }) },
		model: { cachedEntry: () => cached, handle: 1 },
	} as unknown as ListingSession;
}

describe('what is runnable', () => {
	it('is a regular file with one of the five extensions, in any case', () => {
		for (const name of ['a.exe', 'A.BAT', 'a.Cmd', 'a.msi', 'a.LNK', 'v1.2.exe']) {
			expect(isRunnableName(name), name).toBe(true);
		}
		for (const name of ['a.txt', 'exe', '.exe', 'a.exe.txt', 'a.ps1', 'a.com', 'a.exe ']) {
			expect(isRunnableName(name), name).toBe(false);
		}
		expect(isRunnableEntry(entry('a.exe'))).toBe(true);
		expect(isRunnableEntry(entry('a.exe', 'directory'))).toBe(false);
		expect(isRunnableEntry(entry('a.exe', 'symlink'))).toBe(false);
	});

	it('is offered only on Windows with the plugin feature', () => {
		const status = (available: boolean) =>
			({
				features: [{ name: 'runElevated', available, reason: null, message: null }],
			}) as never;
		expect(runAsAdministratorOffered(status(true), 'windows')).toBe(true);
		expect(runAsAdministratorOffered(status(true), 'linux')).toBe(false);
		expect(runAsAdministratorOffered(status(true), undefined)).toBe(false);
		expect(runAsAdministratorOffered(status(false), 'windows')).toBe(false);
		expect(runAsAdministratorOffered(null, 'windows')).toBe(false);
	});

	it('needs exactly one selected, cached, runnable file', () => {
		const one = { kind: 'some', ids: new Set([7]) } as const;
		expect(runnableFileSelected(sessionWith(one, entry('a.exe')), 1)).toBe(true);
		expect(runnableFileSelected(sessionWith(one, entry('a.txt')), 1)).toBe(false);
		expect(runnableFileSelected(sessionWith(one, entry('a.exe', 'directory')), 1)).toBe(false);
		// Not cached: unknown, so not offered.
		expect(runnableFileSelected(sessionWith(one, undefined), 1)).toBe(false);
		expect(runnableFileSelected(sessionWith(one, entry('a.exe')), 2)).toBe(false);
		expect(runnableFileSelected(sessionWith(one, entry('a.exe')), 0)).toBe(false);
		expect(runnableFileSelected(sessionWith({ kind: 'all' }, entry('a.exe')), 1)).toBe(false);
		expect(runnableFileSelected(null, 1)).toBe(false);
	});
});

describe('startRunAsAdministrator', () => {
	it('hands the locations to the plugin', async () => {
		const client = createFakeOpenWithClient();
		const say = vi.fn();
		expect(await startRunAsAdministrator(client, uris, say)).toBe(true);
		expect(client.calls).toEqual([['runElevated', uris]]);
		expect(say).not.toHaveBeenCalled();
	});

	it('is silent when the person declines the prompt', async () => {
		const client = createFakeOpenWithClient({ runElevatedError: { kind: 'cancelled' } });
		const say = vi.fn();
		expect(await startRunAsAdministrator(client, uris, say)).toBe(false);
		expect(say).not.toHaveBeenCalled();
	});

	it('says it could not, without the path, for any other failure', async () => {
		const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
		const say = vi.fn();
		for (const error of [
			{ kind: 'failed', message: 'denied C:\\Tools\\setup.exe' },
			{ kind: 'unsupported' },
		] as const) {
			const client = createFakeOpenWithClient({ runElevatedError: error });
			expect(await startRunAsAdministrator(client, uris, say)).toBe(false);
		}
		expect(say).toHaveBeenCalledTimes(2);
		expect(say).toHaveBeenLastCalledWith(t('runAsAdministrator.failed'));
		expect(t('runAsAdministrator.failed')).toBe('Could not run it as an administrator.');
		expect(String(say.mock.calls)).not.toContain('Tools');
		warn.mockRestore();
	});
});

describe('runRunAsAdministratorCommand', () => {
	const vfs = {
		entryLocation: vi.fn(async (_handle: number, id: number) => ({
			display: 'setup.exe',
			uri: `file:///C:/Tools/${id}.exe`,
		})),
	};

	it('resolves the one selected file through the listing', async () => {
		const client = createFakeOpenWithClient();
		const one = { kind: 'some', ids: new Set([7]) } as const;
		const session = {
			store: { getState: () => ({ selection: one }) },
			model: {
				handle: 1,
				count: 1,
				cachedEntry: () => entry('setup.exe'),
				entries: undefined,
			},
		} as unknown as ListingSession;
		selectedEntries.mockResolvedValue([entry('setup.exe')]);
		await runRunAsAdministratorCommand(session, { client, vfs, say: vi.fn() });
		expect(client.calls).toEqual([['runElevated', ['file:///C:/Tools/7.exe']]]);
	});

	it('takes a given local location, and refuses one that is not local', async () => {
		const client = createFakeOpenWithClient();
		await runRunAsAdministratorCommand(
			null,
			{ client, vfs },
			{ display: 'setup.exe', uri: 'file:///C:/a/setup.exe' },
		);
		await runRunAsAdministratorCommand(
			null,
			{ client, vfs },
			{ display: 'x.exe', uri: 'admin:///C:/a/x.exe' },
		);
		await runRunAsAdministratorCommand(
			null,
			{ client, vfs },
			{ display: 'x.exe', uri: 'sftp://host/x.exe' },
		);
		expect(client.calls).toEqual([['runElevated', ['file:///C:/a/setup.exe']]]);
	});
});
