// The List and Grid switcher for the status bar footer, with the grid's icon size slider
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { GridViewIcon, ListViewIcon } from '../icons/AppIcons';
import { t, tf } from '../i18n/messages';
import {
	GRID_SIZE_MAX,
	GRID_SIZE_MIN,
	GRID_SIZE_STEP,
	useViewState,
	useViewStore,
	type ViewMode,
} from '../browse/viewStore';
import styles from './ViewSwitcher.module.css';

const MODES: Array<{ mode: ViewMode; label: 'view.list' | 'view.grid'; keys: string }> = [
	{ mode: 'list', label: 'view.list', keys: 'Ctrl+2' },
	{ mode: 'grid', label: 'view.grid', keys: 'Ctrl+1' },
];

export function ViewSwitcher() {
	const store = useViewStore();
	const mode = useViewState((state) => state.mode);
	const gridSize = useViewState((state) => state.gridSize);
	return (
		<div className={styles.switcher} role="group" aria-label={t('view.switcher.label')}>
			{mode === 'grid' && (
				<input
					type="range"
					className={styles.slider}
					aria-label={t('view.gridSize')}
					aria-valuetext={tf('view.gridSize.value', { size: gridSize })}
					min={GRID_SIZE_MIN}
					max={GRID_SIZE_MAX}
					step={GRID_SIZE_STEP}
					value={gridSize}
					onChange={(event) => store.getState().setGridSize(Number(event.target.value))}
				/>
			)}
			{MODES.map((entry) => (
				<button
					key={entry.mode}
					type="button"
					className={styles.button}
					aria-pressed={mode === entry.mode}
					aria-label={t(entry.label)}
					title={tf('view.withShortcut', { name: t(entry.label), keys: entry.keys })}
					onClick={() => store.getState().setMode(entry.mode)}
				>
					{entry.mode === 'grid' ? <GridViewIcon /> : <ListViewIcon />}
				</button>
			))}
		</div>
	);
}
