// The Language & region page: the language of the app and the direction it lays out in
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { SegmentedRow } from '@liminal-hq/waypoint-chrome/SettingsShell/SegmentedRow';
import { SelectRow } from '@liminal-hq/waypoint-chrome/SettingsShell/SelectRow';
import { SettingsGroup } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsGroup';
import { SettingsSection } from '@liminal-hq/waypoint-chrome/SettingsShell/SettingsSection';
import { LOCALES, PSEUDO_LOCALES } from '../i18n/locales';
import { t } from '../i18n/messages';
import { useSettingsEditor } from './SettingsEditor';

/** The language choices: the OS default, the languages that ship and, in a developer build, the pseudo-locales. */
export function languageOptions(developer: boolean): { value: string; label: string }[] {
	const tags = developer ? [...LOCALES, ...PSEUDO_LOCALES] : [...LOCALES];
	return [
		{ value: 'system', label: t('settings.language.language.system') },
		...tags.map((tag) => ({ value: tag, label: t(`settings.language.language.${tag}`) })),
	];
}

export function LanguagePage() {
	const { settings, errors, changeSettings } = useSettingsEditor();
	const { locale } = settings;
	const options = languageOptions(import.meta.env.DEV);
	// A saved language this build does not offer (a pseudo-locale in a release build) shows as the default.
	const value = options.some((option) => option.value === locale.language)
		? locale.language
		: 'system';
	return (
		<SettingsSection>
			<SettingsGroup title={t('settings.group.language')}>
				<SelectRow
					label={t('settings.language.language.label')}
					description={t('settings.language.language.description')}
					error={errors.language}
					value={value}
					options={options}
					onChange={(language) =>
						changeSettings('language', (s) => ({ ...s, locale: { ...s.locale, language } }))
					}
				/>
			</SettingsGroup>
			<SettingsGroup title={t('settings.group.direction')}>
				<SegmentedRow
					label={t('settings.language.direction.label')}
					description={t('settings.language.direction.description')}
					error={errors.direction}
					value={locale.direction}
					options={[
						{ value: 'auto', label: t('settings.language.direction.auto') },
						{ value: 'ltr', label: t('settings.language.direction.ltr') },
						{ value: 'rtl', label: t('settings.language.direction.rtl') },
					]}
					onChange={(direction) =>
						changeSettings('direction', (s) => ({ ...s, locale: { ...s.locale, direction } }))
					}
				/>
			</SettingsGroup>
		</SettingsSection>
	);
}
