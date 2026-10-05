// Verifies the System icon set in FileIcon: the Waypoint glyph while loading and where the system has none, the system's picture once loaded, one request per type, and a repaint when the OS look changes
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, render } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { IconGroup } from '@liminal-hq/waypoint-protocol/generated/IconGroup';
import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import { EntryHero } from '../inspector/EntryHero';
import { EntryPreview } from '../inspector/EntryPreview';
import { QuickLookPreview } from '../quicklook/QuickLookPreview';
import type { DetailsClient } from '../services/detailsClient';
import { configureSystemIcons, systemImageCount } from '../icons/systemIcons';
import {
	createFakeSystemIconsClient,
	type FakeSystemIcons,
} from '../services/fakeSystemIconsClient';
import { FileIcon } from './FileIcon';
import { IconPictures } from './IconPictures';

const root = document.documentElement;

let fake: FakeSystemIcons;

/** Lets the plugin's first answers (status and look) arrive and the icons that were waiting for them re-render. */
async function settleStatus(): Promise<void> {
	await act(async () => {
		for (let turn = 0; turn < 5; turn += 1) await Promise.resolve();
	});
}

function setup(options?: Parameters<typeof createFakeSystemIconsClient>[0]): FakeSystemIcons {
	fake = createFakeSystemIconsClient(options);
	configureSystemIcons(fake);
	return fake;
}

beforeEach(() => {
	root.dataset.iconTheme = 'system';
	setup();
});

afterEach(() => {
	cleanup();
	configureSystemIcons(null);
	for (const key of ['iconTheme', 'theme']) delete root.dataset[key];
});

const glyph = (container: HTMLElement) =>
	container.querySelector('svg:not([data-system])[data-group]');
const picture = (container: HTMLElement) => container.querySelector('svg[data-system] image');

describe('FileIcon in the System set', () => {
	it('draws the Waypoint glyph until the plugin has answered and the picture has loaded, never nothing', async () => {
		const { container } = render(<FileIcon group="pdf" name="report.pdf" />);
		// Before the plugin answers.
		expect(glyph(container)).not.toBeNull();
		expect(fake.probed).toEqual([]);
		await settleStatus();
		// Asked for, not here yet.
		expect(fake.probed).toEqual(['fake://ext/pdf?size=16&scale=1&theme=Adwaita&tone=light']);
		expect(glyph(container)).not.toBeNull();
		expect(picture(container)).toBeNull();
		await act(async () => fake.settle(true));
		expect(glyph(container)).toBeNull();
		expect(picture(container)?.getAttribute('href')).toBe(fake.probed[0]);
	});

	it('keeps the Waypoint glyph for good where the system has no icon for the type', async () => {
		const { container } = render(<FileIcon group="other" name="data.zzz" />);
		await settleStatus();
		await act(async () => fake.settle(false));
		expect(glyph(container)).not.toBeNull();
		expect(picture(container)).toBeNull();
	});

	it('is sized by the same class as the glyph, and decorative', async () => {
		const { container } = render(<FileIcon group="pdf" name="a.pdf" className="big" size={64} />);
		await settleStatus();
		await act(async () => fake.settle(true));
		const svg = container.querySelector('svg[data-system]')!;
		expect(svg.classList.contains('big')).toBe(true);
		expect(svg.getAttribute('aria-hidden')).toBe('true');
		expect(svg.getAttribute('focusable')).toBe('false');
		expect(fake.probed[0]).toContain('size=64');
	});

	it('asks for a folder by its kind, and a standard folder by its own', async () => {
		render(
			<>
				<FileIcon group="folder" name="src" />
				<FileIcon group="folder" name="Downloads" special="downloads" />
				<FileIcon group="folder" name="Projects" special="projects" />
			</>,
		);
		await settleStatus();
		expect(fake.probed.map((url) => url.split('?')[0]).sort()).toEqual([
			'fake://folder/downloads',
			'fake://folder/plain',
		]);
	});

	it('asks for a handful of pictures to draw a listing of thousands of files', async () => {
		const groups: Array<[IconGroup, string]> = [
			['pdf', 'pdf'],
			['image', 'png'],
			['document', 'odt'],
			['archive', 'zip'],
			['text', 'txt'],
		];
		const rows = Array.from({ length: 10_000 }, (_, index) => {
			const [group, extension] = groups[index % groups.length]!;
			return <FileIcon key={index} group={group} name={`file-${index}.${extension}`} />;
		});
		render(<>{rows}</>);
		await settleStatus();
		expect(fake.probed).toHaveLength(5);
		expect(systemImageCount()).toBe(5);
		await act(async () => fake.settle(true));
		expect(document.querySelectorAll('svg[data-system]')).toHaveLength(10_000);
		// Nothing was asked again once the pictures were there.
		expect(fake.probed).toHaveLength(5);
	}, 30_000);

	it('asks for the group’s stand-in type when the name has no extension', async () => {
		render(<FileIcon group="code" name="Makefile" />);
		await settleStatus();
		expect(fake.probed[0]).toContain('fake://mime/text/x-csrc?');
	});

	it('draws only the Waypoint glyph, and asks nobody, where the system supplies no icons', async () => {
		setup({
			status: {
				typeIcons: { available: false, reason: 'no icon theme' },
				folderIcons: { available: false, reason: 'no icon theme' },
			},
		});
		const { container } = render(
			<>
				<FileIcon group="pdf" name="a.pdf" />
				<FileIcon group="folder" name="d" />
			</>,
		);
		await settleStatus();
		expect(fake.probed).toEqual([]);
		expect(container.querySelectorAll('svg:not([data-system])[data-group]')).toHaveLength(2);
	});

	it('draws the glyph when the plugin does not answer at all', async () => {
		const failing = setup();
		failing.failStatus();
		const { container } = render(<FileIcon group="pdf" name="a.pdf" />);
		await settleStatus();
		expect(failing.probed).toEqual([]);
		expect(glyph(container)).not.toBeNull();
	});

	it('uses the system for folders and the glyph for files when only folders are supplied', async () => {
		setup({ status: { typeIcons: { available: false, reason: 'no theme for types' } } });
		const { container } = render(
			<>
				<FileIcon group="folder" name="d" />
				<FileIcon group="pdf" name="a.pdf" />
			</>,
		);
		await settleStatus();
		expect(fake.probed).toHaveLength(1);
		expect(fake.probed[0]).toContain('folder/plain');
		await act(async () => fake.settle(true));
		expect(container.querySelectorAll('svg[data-system]')).toHaveLength(1);
		expect(container.querySelectorAll('svg:not([data-system])[data-group="pdf"]')).toHaveLength(1);
	});

	it('repaints when the OS icon theme changes: the plugin forgets its icons and every icon is asked for again', async () => {
		const { container } = render(<FileIcon group="pdf" name="a.pdf" />);
		await settleStatus();
		await act(async () => fake.settle(true));
		expect(picture(container)?.getAttribute('href')).toContain('theme=Adwaita');
		await act(async () => {
			fake.changeLook({ theme: 'Papirus' });
			for (let turn = 0; turn < 5; turn += 1) await Promise.resolve();
		});
		expect(fake.refreshed).toBe(1);
		// The old picture is not shown for the new theme: the glyph holds until the new one loads.
		expect(glyph(container)).not.toBeNull();
		const url = fake.probed.at(-1)!;
		expect(url).toContain('theme=Papirus');
		expect(url).toContain('v=1');
		await act(async () => fake.settle(true));
		expect(picture(container)?.getAttribute('href')).toBe(url);
	});

	it('repaints when the OS colour mode changes, and ignores a change that is neither', async () => {
		render(<FileIcon group="pdf" name="a.pdf" />);
		await settleStatus();
		await act(async () => fake.settle(true));
		await act(async () => {
			fake.changeLook({});
			for (let turn = 0; turn < 5; turn += 1) await Promise.resolve();
		});
		expect(fake.refreshed).toBe(0);
		await act(async () => {
			fake.changeLook({ scheme: 'dark' });
			for (let turn = 0; turn < 5; turn += 1) await Promise.resolve();
		});
		expect(fake.refreshed).toBe(1);
		expect(fake.probed.at(-1)).toContain('v=1');
	});

	it('asks again, from the plugin’s cache, when the window’s own colour mode changes', async () => {
		const { container } = render(<FileIcon group="pdf" name="a.pdf" />);
		await settleStatus();
		await act(async () => fake.settle(true));
		expect(fake.probed).toHaveLength(1);
		await act(async () => {
			root.dataset.theme = 'dark';
		});
		await waitForProbe(2);
		expect(fake.probed.at(-1)).toMatch(/tone=dark$/);
		await act(async () => fake.settle(true));
		expect(picture(container)?.getAttribute('href')).toMatch(/tone=dark$/);
		// Nothing but the tone changed: no refresh of the plugin.
		expect(fake.refreshed).toBe(0);
	});

	it('is the same Waypoint glyph it always was under the other sets, and never calls the plugin', async () => {
		root.dataset.iconTheme = 'waypoint';
		const status = vi.spyOn(fake, 'status');
		const { container } = render(<FileIcon group="pdf" name="a.pdf" />);
		await settleStatus();
		expect(status).not.toHaveBeenCalled();
		expect(fake.probed).toEqual([]);
		expect(glyph(container)).not.toBeNull();
	});

	it('draws the System set in a preview whatever the window is set to', async () => {
		root.dataset.iconTheme = 'portage';
		const { container } = render(<FileIcon group="pdf" name="a.pdf" theme="system" />);
		await settleStatus();
		expect(fake.probed).toHaveLength(1);
		await act(async () => fake.settle(true));
		expect(container.querySelector('svg[data-system]')).not.toBeNull();
	});

	it('stops listening for the OS look when the last icon goes', async () => {
		const { unmount } = render(<FileIcon group="pdf" name="a.pdf" />);
		await settleStatus();
		expect(fake.listenerCount).toBe(1);
		unmount();
		expect(fake.listenerCount).toBe(0);
	});
});

describe('FileIcon for a file that carries its own icon', () => {
	const source = { handle: 3, id: 9, modifiedMs: 1234 };
	const fileUrl = 'fake://file/3-9?size=16&scale=1&m=1234';
	const typeUrl = 'fake://ext/exe?size=16&scale=1&theme=Adwaita&tone=light';

	it('asks for the file’s own icon alone, and draws it once it is there', async () => {
		const { container } = render(<FileIcon group="executable" name="Setup.EXE" source={source} />);
		await settleStatus();
		// The type's icon is not asked for while the file's may still come.
		expect(fake.probed).toEqual([fileUrl]);
		expect(glyph(container)).not.toBeNull();
		await act(async () => fake.settleUrl(fileUrl, true));
		expect(picture(container)?.getAttribute('href')).toBe(fileUrl);
		expect(fake.probed).toEqual([fileUrl]);
	});

	it('draws the type’s icon when the system has none for the file', async () => {
		const { container } = render(<FileIcon group="executable" name="app.exe" source={source} />);
		await settleStatus();
		await act(async () => fake.settleUrl(fileUrl, false));
		expect(fake.probed).toEqual([fileUrl, typeUrl]);
		expect(glyph(container)).not.toBeNull();
		await act(async () => fake.settleUrl(typeUrl, true));
		expect(picture(container)?.getAttribute('href')).toBe(typeUrl);
	});

	it('is a new address when the file changes, so the new icon is asked for', async () => {
		const { rerender } = render(<FileIcon group="executable" name="app.exe" source={source} />);
		await settleStatus();
		rerender(
			<FileIcon group="executable" name="app.exe" source={{ ...source, modifiedMs: 5678 }} />,
		);
		await settleStatus();
		expect(fake.probed).toEqual([fileUrl, 'fake://file/3-9?size=16&scale=1&m=5678']);
	});

	it('is drawn from the file as a background picture where the icons are pictures (the grid), and the type’s icon is the shared picture for every other file', async () => {
		const { container } = render(
			<IconPictures>
				<FileIcon group="executable" name="Setup.EXE" source={source} />
				<FileIcon group="document" name="notes.txt" source={{ ...source, id: 10 }} />
			</IconPictures>,
		);
		await settleStatus();
		const txtUrl = 'fake://ext/txt?size=16&scale=1&theme=Adwaita&tone=light';
		expect(fake.probed).toEqual([fileUrl, txtUrl]);
		await act(async () => fake.settleUrl(fileUrl, true));
		await act(async () => fake.settleUrl(txtUrl, true));
		const drawn = [...container.querySelectorAll<HTMLElement>('[data-icon-picture][data-system]')];
		expect(drawn.map((icon) => icon.style.backgroundImage)).toEqual([
			`url("${fileUrl}")`,
			`url("${txtUrl}")`,
		]);
		expect(container.querySelector('svg[data-system]')).toBeNull();
	});

	it('falls back from the file’s icon to the type’s to the glyph where the icons are pictures', async () => {
		const { container } = render(
			<IconPictures>
				<FileIcon group="executable" name="app.exe" source={source} />
			</IconPictures>,
		);
		await settleStatus();
		expect(container.querySelector('[data-system]')).toBeNull();
		await act(async () => fake.settleUrl(fileUrl, false));
		expect(fake.probed).toEqual([fileUrl, typeUrl]);
		expect(container.querySelector('[data-system]')).toBeNull();
		await act(async () => fake.settleUrl(typeUrl, true));
		expect(
			container.querySelector<HTMLElement>('[data-icon-picture][data-system]')?.style
				.backgroundImage,
		).toBe(`url("${typeUrl}")`);
	});

	it('asks for every kind that carries an icon, and for no other', async () => {
		render(
			<>
				<FileIcon group="executable" name="a.exe" source={{ ...source, id: 1 }} />
				<FileIcon group="other" name="b.lnk" source={{ ...source, id: 2 }} />
				<FileIcon group="image" name="c.ico" source={{ ...source, id: 3 }} />
				<FileIcon group="pdf" name="d.pdf" source={{ ...source, id: 4 }} />
				<FileIcon group="executable" name="Makefile" source={{ ...source, id: 5 }} />
				<FileIcon group="folder" name="e.exe" source={{ ...source, id: 6 }} />
			</>,
		);
		await settleStatus();
		const asked = fake.probed.map((url) => url.split('?')[0]).sort();
		expect(asked).toEqual(
			[
				'fake://file/3-1',
				'fake://file/3-2',
				'fake://file/3-3',
				'fake://ext/pdf',
				'fake://folder/plain',
				'fake://mime/application/x-executable',
			].sort(),
		);
	});

	it('goes by the type alone when the entry is not named', async () => {
		render(<FileIcon group="executable" name="a.exe" />);
		await settleStatus();
		expect(fake.probed).toEqual([typeUrl]);
	});

	it('asks for nothing where the system supplies no icons for types', async () => {
		setup({ status: { typeIcons: { available: false, reason: 'none' } } });
		render(<FileIcon group="executable" name="a.exe" source={source} />);
		await settleStatus();
		expect(fake.probed).toEqual([]);
	});

	const exeEntry = {
		id: 9,
		name: 'app.exe',
		kind: 'file',
		linkTarget: null,
		group: 'executable',
		size: 10,
		modifiedMs: 1234,
		hidden: false,
	} as unknown as Entry;

	it('is drawn from the file in the Inspector’s Properties hero, at its size', async () => {
		render(<EntryHero handle={3} entry={exeEntry} size={10} folder={false} />);
		await settleStatus();
		expect(fake.probed).toEqual(['fake://file/3-9?size=64&scale=1&m=1234']);
	});

	it('is drawn from the file in the Inspector’s Preview tab, by its own extension, at its size', async () => {
		render(<EntryPreview client={null} handle={3} entry={exeEntry} details={null} />);
		await settleStatus();
		expect(fake.probed).toEqual(['fake://file/3-9?size=64&scale=1&m=1234']);
	});

	it('is drawn from the file in Quick Look’s facts panel, at the size bucket that holds it (160 px asks for 256)', async () => {
		render(
			<QuickLookPreview
				entry={exeEntry}
				handle={3}
				client={{} as DetailsClient}
				loader={null}
				hourCycle={undefined}
			/>,
		);
		await settleStatus();
		expect(fake.probed).toEqual(['fake://file/3-9?size=256&scale=1&m=1234']);
	});
});

async function waitForProbe(count: number): Promise<void> {
	await act(async () => {
		for (let turn = 0; turn < 10 && fake.probed.length < count; turn += 1) await Promise.resolve();
	});
	expect(fake.probed.length).toBeGreaterThanOrEqual(count);
}
