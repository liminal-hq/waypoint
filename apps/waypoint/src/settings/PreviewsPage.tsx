// The Previews & thumbnails page: whether thumbnails are shown and the largest file one is made of
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { NumberRow } from '@liminal-hq/waypoint-chrome/SettingsShell/NumberRow';
import { SettingsGroup } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsGroup';
import { SettingsSection } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsSection';
import { ToggleRow } from '@liminal-hq/waypoint-chrome/SettingsShell/ToggleRow';
import { t, tf } from '../i18n/messages';
import { PREVIEW_MAX_MB_MAX, PREVIEW_MAX_MB_MIN } from '../services/settingsClient';
import { useSettingsEditor } from './SettingsEditor';
import { ServerPreviewsGroup } from './ServerPreviews';

/**
 * Thumbnails over the `previews` settings. Where the plugin reports itself unavailable the options
 * are not shown: the page says why, in the plugin's own words, instead (the Services panel lists
 * the same reason). "Measure Home when Overview opens" is always shown; the other `previews` settings (folder peeks, hover to peek) arrive
 * with the features they switch.
 */
export function PreviewsPage() {
	const { settings, thumbnails, errors, changeSettings } = useSettingsEditor();
	const unavailable = thumbnails !== null && !thumbnails.available;
	const previews = settings.previews;
	return (
		<SettingsSection
			description={
				unavailable
					? tf('settings.previews.unavailable', {
							reason: thumbnails.reason?.message ?? t('settings.previews.unavailable.noReason'),
						})
					: undefined
			}
		>
			{!unavailable && (
				<SettingsGroup title={t('settings.group.thumbnails')}>
					<ToggleRow
						label={t('settings.previews.show.label')}
						description={t('settings.previews.show.description')}
						error={errors.thumbnails}
						checked={previews.thumbnails}
						onChange={(value) =>
							changeSettings('thumbnails', (s) => ({
								...s,
								previews: { ...s.previews, thumbnails: value },
							}))
						}
					/>
					<NumberRow
						label={t('settings.previews.max.label')}
						description={t('settings.previews.max.description')}
						error={errors.thumbnailMax}
						value={previews.maxFileMb}
						min={PREVIEW_MAX_MB_MIN}
						max={PREVIEW_MAX_MB_MAX}
						step={10}
						unit={t('settings.previews.max.unit')}
						commitOn="commit"
						disabled={!previews.thumbnails}
						onChange={(maxFileMb) =>
							changeSettings('thumbnailMax', (s) => ({
								...s,
								previews: { ...s.previews, maxFileMb },
							}))
						}
					/>
				</SettingsGroup>
			)}
			{!unavailable && <ServerPreviewsGroup />}
			<SettingsGroup title={t('settings.group.overview')}>
				<ToggleRow
					label={t('settings.previews.measureHome.label')}
					description={t('settings.previews.measureHome.description')}
					error={errors.measureHomeOnOpen}
					checked={previews.measureHomeOnOpen}
					onChange={(value) =>
						changeSettings('measureHomeOnOpen', (s) => ({
							...s,
							previews: { ...s.previews, measureHomeOnOpen: value },
						}))
					}
				/>
			</SettingsGroup>
		</SettingsSection>
	);
}
