// Verifies that a scrolling view holds back pictures not drawn yet and keeps the ones already drawn
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { Thumbnail } from './Thumbnail';
import type { ThumbnailLoader } from './thumbnailLoader';
import { createThumbnailPause, ThumbnailPauseContext } from './thumbnailPause';

afterEach(cleanup);

/** A loader that has a picture for every key. */
const loader = {
	subscribe: () => () => undefined,
	urlOf: (key: string) => `thumb://localhost/${key}.png`,
} as unknown as ThumbnailLoader<{ key: string }>;

function draw(thumbKey: string, pause: ReturnType<typeof createThumbnailPause>) {
	return (
		<ThumbnailPauseContext.Provider value={pause}>
			<Thumbnail loader={loader} thumbKey={thumbKey} group="image" name="a.png" />
		</ThumbnailPauseContext.Provider>
	);
}

describe('a thumbnail in a scrolling view', () => {
	it('waits for the scroll to stop before drawing a new picture, and keeps one it has drawn', () => {
		const pause = createThumbnailPause();
		act(() => pause.set(true));
		const view = render(draw('a', pause));
		expect(view.container.querySelector('img')).toBeNull();
		act(() => pause.set(false));
		const picture = view.container.querySelector('img')!;
		expect(picture).toHaveAttribute('src', 'thumb://localhost/a.png');
		expect(picture).toHaveAttribute('decoding', 'async');
		fireEvent.load(picture);
		// Drawn: a scroll does not take it away.
		act(() => pause.set(true));
		expect(view.container.querySelector('img')).toBe(picture);
		// The frame takes another item's picture mid-scroll: the old one goes and the new one waits.
		view.rerender(draw('b', pause));
		expect(view.container.querySelector('img')).toBeNull();
		act(() => pause.set(false));
		expect(view.container.querySelector('img')).toHaveAttribute('src', 'thumb://localhost/b.png');
	});
});
