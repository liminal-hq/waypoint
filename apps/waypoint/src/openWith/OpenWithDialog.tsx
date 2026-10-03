// The application chooser: the installed applications for some locations, filtered as the person types
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { App } from '@liminal-hq/plugin-mime-apps';
import { Dialog } from '@liminal-hq/waypoint-chrome/Dialog/Dialog';
import { DialogActions, DialogButton } from '@liminal-hq/waypoint-chrome/Dialog/DialogActions';
import { useMemo, useState, type KeyboardEvent } from 'react';
import { t, tn } from '../i18n/messages';
import { AppIcon } from './OpenWithIcons';
import type { ChooserRequest } from './openWithChooserStore';
import styles from './OpenWithDialog.module.css';

export interface OpenWithDialogProps {
	request: ChooserRequest;
	iconUrl: (appId: string) => string;
	/** Called with the application that was chosen. */
	onChoose: (app: App) => void;
	onCancel: () => void;
}

/** The applications to list, by section, for the request's scope; `filter` is matched against names, ignoring case. */
export function chooserSections(
	request: ChooserRequest,
	filter: string,
): Array<{ key: 'recommended' | 'others'; apps: App[] }> {
	const { handlers, scope } = request;
	const needle = filter.trim().toLowerCase();
	const keep = (app: App) => needle === '' || app.name.toLowerCase().includes(needle);
	const recommended =
		scope === 'all' ? [handlers.default, ...handlers.recommended].flatMap((app) => app ?? []) : [];
	return [
		{ key: 'recommended' as const, apps: recommended.filter(keep) },
		{ key: 'others' as const, apps: handlers.others.filter(keep) },
	].filter((section) => section.apps.length > 0);
}

/**
 * Lists the applications; choosing one (a click, or Enter on the first match while typing) opens
 * the locations in it. Esc cancels. The filter has focus when the dialog opens, because the list of
 * installed applications is long.
 */
export function OpenWithDialog({ request, iconUrl, onChoose, onCancel }: OpenWithDialogProps) {
	const [filter, setFilter] = useState('');
	const sections = useMemo(() => chooserSections(request, filter), [request, filter]);
	const first = sections[0]?.apps[0];

	const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
		if (event.key !== 'Enter' || event.nativeEvent.isComposing || !first) return;
		event.preventDefault();
		onChoose(first);
	};

	return (
		<Dialog
			open
			size="medium"
			title={t('openWith.dialog.title')}
			description={tn('openWith.dialog.description', request.uris.length)}
			initialFocus="[data-openwith-filter]"
			onClose={onCancel}
			footer={
				<DialogActions>
					<DialogButton onClick={onCancel}>{t('openWith.cancel')}</DialogButton>
				</DialogActions>
			}
		>
			<div className={styles.body}>
				<label className={styles.field}>
					<span className={styles.label}>{t('openWith.filter.label')}</span>
					<input
						data-openwith-filter=""
						className={styles.input}
						type="search"
						spellCheck={false}
						autoComplete="off"
						value={filter}
						onChange={(event) => setFilter(event.target.value)}
						onKeyDown={onKeyDown}
					/>
				</label>
				<div className={styles.apps} role="group" aria-label={t('openWith.apps.label')}>
					{sections.length === 0 && <p className={styles.empty}>{t('openWith.empty')}</p>}
					{sections.map(({ key, apps }) => (
						<section key={key} className={styles.section} aria-label={t(`openWith.section.${key}`)}>
							<h3 className={styles.heading}>{t(`openWith.section.${key}`)}</h3>
							<ul className={styles.list}>
								{apps.map((app) => (
									<li key={app.id}>
										<button
											type="button"
											className={styles.app}
											title={app.execHint ?? undefined}
											onClick={() => onChoose(app)}
										>
											<AppIcon src={iconUrl(app.id)} />
											<span className={styles.name}>{app.name}</span>
										</button>
									</li>
								))}
							</ul>
						</section>
					))}
				</div>
			</div>
		</Dialog>
	);
}
