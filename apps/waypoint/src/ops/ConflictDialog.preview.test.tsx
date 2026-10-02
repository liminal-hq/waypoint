// Verifies the previews in the conflict dialog: thumbnails, the identical notice, the text diff in words and signs, the row cap, and the dialog working as before when a preview cannot load
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Conflict } from '@liminal-hq/waypoint-protocol/generated/Conflict';
import type { ConflictPreview } from '@liminal-hq/waypoint-protocol/generated/ConflictPreview';
import type { DiffLine } from '@liminal-hq/waypoint-protocol/generated/DiffLine';
import type { PreviewKind } from '@liminal-hq/waypoint-protocol/generated/PreviewKind';
import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { TimeFormatProvider } from '../browse/TimeFormatContext';
import { FakeTimeFormatClient } from '../services/fakeTimeFormatClient';
import { fileLocation } from '../services/fakeVfsClient';
import { fakeJobSnapshot } from '../trash/fakeTrashClient';
import { conflictFor } from '../test/opsHarness';
import { createFakeThumbnailsClient } from '../thumbnails/fakeThumbnailsClient';
import { ThumbnailsProvider } from '../thumbnails/ThumbnailsContext';
import { ConflictDialog } from './ConflictDialog';
import { AUTO_PREVIEWS, DIFF_ROW_CAP } from './conflictPreviewModel';

afterEach(cleanup);

function previewOf(conflict: Conflict, kind: PreviewKind): ConflictPreview {
	return {
		existing: {
			location: conflict.existing,
			size: conflict.existingSize,
			modifiedMs: conflict.existingModifiedMs,
		},
		incoming: {
			location: conflict.source,
			size: conflict.sourceSize,
			modifiedMs: conflict.sourceModifiedMs,
		},
		kind,
	};
}

const diffKind = (lines: DiffLine[], more = 0): PreviewKind => ({
	type: 'text',
	diff: {
		lines,
		added: lines.filter((l) => l.op === 'added').length,
		removed: lines.filter((l) => l.op === 'removed').length,
		more,
		approximate: false,
		lossy: false,
	},
});

function mount(
	conflicts: Conflict[],
	loadPreview?: (conflict: Conflict) => Promise<ConflictPreview>,
	thumbnails?: ReturnType<typeof createFakeThumbnailsClient>,
) {
	const handlers = { onContinue: vi.fn(), onCancelJob: vi.fn(), onLater: vi.fn() };
	render(
		<TimeFormatProvider client={new FakeTimeFormatClient()}>
			<ThumbnailsProvider client={thumbnails}>
				<ConflictDialog
					job={{
						...fakeJobSnapshot(1, { kind: 'copy' }, { state: 'queued' }),
						destination: fileLocation('/dest'),
					}}
					conflicts={conflicts}
					loadPreview={loadPreview}
					{...handlers}
				/>
			</ThumbnailsProvider>
		</TimeFormatProvider>,
	);
	return handlers;
}

const dialog = () => screen.getByRole('dialog');

describe('which file is newer or larger', () => {
	it('says the incoming file is larger, in words', () => {
		mount([conflictFor('a.txt', { sourceSize: 4096, existingSize: 1024 })]);
		expect(within(dialog()).getByText('Larger than the existing one')).toBeInTheDocument();
		expect(within(dialog()).getByText('Newer than the existing one')).toBeInTheDocument();
	});
});

describe('the previews', () => {
	it('say nothing and ask for nothing without a client', () => {
		mount([conflictFor('a.txt')]);
		expect(within(dialog()).queryByRole('button', { name: /compare|differences/i })).toBeNull();
		expect(within(dialog()).getAllByRole('row')).toHaveLength(2);
	});

	it('say identical contents, and that Skip is probably wanted, without choosing it', async () => {
		const conflict = conflictFor('a.txt');
		const handlers = mount([conflict], async () => previewOf(conflict, { type: 'identical' }));
		expect(
			await within(dialog()).findByText('Identical contents. Skip is probably what you want.'),
		).toBeInTheDocument();
		// The default is untouched: nothing chosen, Continue off.
		expect(within(dialog()).getByRole('combobox', { name: 'Choice for a.txt' })).toHaveValue('');
		expect(within(dialog()).getByRole('button', { name: 'Continue' })).toBeDisabled();
		expect(handlers.onContinue).not.toHaveBeenCalled();
	});

	it('count the changes and show the diff when asked, with words for a screen reader', async () => {
		const conflict = conflictFor('a.txt');
		const lines: DiffLine[] = [
			{ op: 'context', oldLine: 1, newLine: 1, text: 'keep' },
			{ op: 'removed', oldLine: 2, text: 'old' },
			{ op: 'added', newLine: 2, text: 'new' },
			{ op: 'gap', lines: 12 },
		];
		mount([conflict], async () => previewOf(conflict, diffKind(lines)));
		expect(await within(dialog()).findByText('1 line added, 1 line removed')).toBeInTheDocument();
		expect(within(dialog()).queryByRole('group', { name: 'Differences in a.txt' })).toBeNull();

		const user = userEvent.setup();
		const show = within(dialog()).getByRole('button', { name: 'Show differences in a.txt' });
		expect(show).toHaveAttribute('aria-expanded', 'false');
		await user.click(show);
		const diff = within(dialog()).getByRole('group', { name: 'Differences in a.txt' });
		expect(within(diff).getByText('Removed, line 2:')).toBeInTheDocument();
		expect(within(diff).getByText('Added, line 2:')).toBeInTheDocument();
		expect(within(diff).getByText('Unchanged, line 1:')).toBeInTheDocument();
		expect(within(diff).getByText('old')).toBeInTheDocument();
		expect(within(diff).getByText('new')).toBeInTheDocument();
		expect(within(diff).getByText('12 unchanged lines')).toBeInTheDocument();
		// The signs are drawn for the eye and hidden from a screen reader.
		expect(within(diff).getByText('+')).toHaveAttribute('aria-hidden', 'true');
		expect(within(diff).getByText('−')).toHaveAttribute('aria-hidden', 'true');
		const hide = within(dialog()).getByRole('button', { name: 'Hide differences in a.txt' });
		expect(hide).toHaveAttribute('aria-expanded', 'true');
		await user.click(hide);
		expect(within(dialog()).queryByRole('group', { name: 'Differences in a.txt' })).toBeNull();
	});

	it('draw at most the row cap and say how many lines were left out', async () => {
		const conflict = conflictFor('a.txt');
		const lines: DiffLine[] = Array.from({ length: DIFF_ROW_CAP + 30 }, (_, i) => ({
			op: 'added',
			newLine: i + 1,
			text: `line ${i + 1}`,
		}));
		mount([conflict], async () => previewOf(conflict, diffKind(lines, 20)));
		const user = userEvent.setup();
		await user.click(
			await within(dialog()).findByRole('button', { name: 'Show differences in a.txt' }),
		);
		const diff = within(dialog()).getByRole('group', { name: 'Differences in a.txt' });
		expect(within(diff).queryByText('line 200')).toBeInTheDocument();
		expect(within(diff).queryByText('line 201')).toBeNull();
		expect(within(diff).getByText('50 more lines')).toBeInTheDocument();
	});

	it('say when the files are not text, too large, or the same text', async () => {
		const binary = conflictFor('b.bin');
		const large = conflictFor('l.txt');
		const same = conflictFor('s.txt');
		const kinds = new Map<string, PreviewKind>([
			[binary.source.uri, { type: 'binary' }],
			[large.source.uri, { type: 'tooLarge' }],
			[same.source.uri, diffKind([])],
		]);
		mount([binary, large, same], async (c) => previewOf(c, kinds.get(c.source.uri)!));
		expect(await within(dialog()).findByText(/not text files/)).toBeInTheDocument();
		expect(await within(dialog()).findByText('Too large to compare here.')).toBeInTheDocument();
		expect(
			await within(dialog()).findByText('The text is the same, so there is nothing to show.'),
		).toBeInTheDocument();
	});

	it('leave the dialog as it was when a preview fails or cannot be made', async () => {
		const failing = conflictFor('a.txt');
		const unavailable = conflictFor('u.txt');
		const load = vi.fn(async (c: Conflict) => {
			if (c === failing) throw new Error('read failed');
			return previewOf(c, { type: 'unavailable' });
		});
		mount([failing, unavailable], load);
		await waitFor(() => expect(load).toHaveBeenCalledTimes(2));
		await act(async () => {});
		expect(within(dialog()).queryByText('Comparing the files…')).toBeNull();
		expect(within(dialog()).queryByRole('button', { name: /differences/i })).toBeNull();
		// Both rows still answer as usual.
		const user = userEvent.setup();
		await user.selectOptions(
			within(dialog()).getByRole('combobox', { name: 'Choice for a.txt' }),
			'skip',
		);
		expect(within(dialog()).getByText('1 of 2 answered')).toBeInTheDocument();
	});

	it('show that they are loading, and never ask for a folder or a clash inside the batch', async () => {
		const file = conflictFor('a.txt');
		let release: (p: ConflictPreview) => void = () => undefined;
		const load = vi.fn(
			() =>
				new Promise<ConflictPreview>((resolve) => {
					release = resolve;
				}),
		);
		mount(
			[
				file,
				conflictFor('d', { kind: 'folderOverFolder' }),
				conflictFor('b', { withinBatch: true }),
			],
			load,
		);
		expect(await within(dialog()).findByText('Comparing the files…')).toBeInTheDocument();
		expect(load).toHaveBeenCalledTimes(1);
		await act(async () => release(previewOf(file, { type: 'identical' })));
		expect(await within(dialog()).findByText(/Identical contents/)).toBeInTheDocument();
	});

	it('read only a few at a time and leave the rest to a button, which asks at once', async () => {
		const conflicts = Array.from({ length: AUTO_PREVIEWS + 3 }, (_, i) => conflictFor(`f${i}.txt`));
		const load = vi.fn(async (c: Conflict) => previewOf(c, { type: 'identical' }));
		mount(conflicts, load);
		await waitFor(() => expect(load).toHaveBeenCalledTimes(AUTO_PREVIEWS));
		const user = userEvent.setup();
		const extra = within(dialog()).getByRole('button', {
			name: `Compare the files named f${AUTO_PREVIEWS}.txt`,
		});
		await user.click(extra);
		await waitFor(() => expect(load).toHaveBeenCalledTimes(AUTO_PREVIEWS + 1));
		expect(load).toHaveBeenLastCalledWith(conflicts[AUTO_PREVIEWS]);
	});
});

describe('the thumbnails', () => {
	it('are asked for both files of a clash and drawn over the icons when they arrive', async () => {
		const thumbnails = createFakeThumbnailsClient();
		const conflict = conflictFor('pic.png');
		mount([conflict], undefined, thumbnails);
		await waitFor(() => expect(thumbnails.batches).toHaveLength(1));
		expect(thumbnails.batches[0]!.keys.sort()).toEqual(
			[conflict.existing.uri, conflict.source.uri].sort(),
		);
		await act(async () => thumbnails.ready(conflict.source.uri, 'thumb://localhost/in.png'));
		const pictures = dialog().querySelectorAll('img');
		expect(pictures).toHaveLength(1);
		expect(pictures[0]).toHaveAttribute('src', 'thumb://localhost/in.png');
		expect(pictures[0]).toHaveAttribute('alt', '');
	});

	it('are not asked for a folder, and the icon stays where there is no client', async () => {
		const thumbnails = createFakeThumbnailsClient();
		mount([conflictFor('d', { kind: 'folderOverFolder' })], undefined, thumbnails);
		await act(async () => {});
		expect(thumbnails.batches).toHaveLength(0);
		cleanup();
		mount([conflictFor('a.png')]);
		expect(dialog().querySelectorAll('img')).toHaveLength(0);
		expect(dialog().querySelectorAll('svg').length).toBeGreaterThan(0);
	});
});
