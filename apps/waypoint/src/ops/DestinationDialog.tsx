// The destination dialog: a typed path checked by Rust, and places, favourites, open tabs and recent folders to choose from
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Dialog } from '@liminal-hq/waypoint-chrome/Dialog/Dialog';
import { DialogActions, DialogButton } from '@liminal-hq/waypoint-chrome/Dialog/DialogActions';
import { useEffect, useId, useRef, useState, type KeyboardEvent } from 'react';
import { t, tf, type MessageId } from '../i18n/messages';
import type { Location } from '../services/opsClient';
import type { VfsClient } from '../services/vfsClient';
import { checkDestination, type DestinationCheck } from './destinationModel';
import type { DestinationOptions } from './destinationStore';
import styles from './DestinationDialog.module.css';

/** A folder offered to choose, under the label a person knows it by. */
export interface DestinationChoice {
	label: string;
	location: Location;
}

/** The lists the dialog offers, each left out when empty. */
export interface DestinationChoices {
	places: DestinationChoice[];
	favourites: DestinationChoice[];
	tabs: DestinationChoice[];
	recent: DestinationChoice[];
}

export interface DestinationDialogProps {
	options: DestinationOptions;
	vfs: Pick<VfsClient, 'parseLocation' | 'checkFolder'>;
	choices: DestinationChoices;
	/**
	 * Makes a folder inside `parent` through the queue and resolves to it; rejects with the reason
	 * in words. Without it the dialog has no New Folder… button.
	 */
	createFolder?: ((parent: Location) => Promise<Location>) | undefined;
	/** How long typing pauses before the folder is checked. */
	debounceMs?: number;
	onConfirm: (destination: Location) => void;
	onCancel: () => void;
}

type Status =
	{ state: 'checking' } | { state: 'working' } | { state: 'made'; name: string } | DestinationCheck;

const SECTIONS: Array<{ key: keyof DestinationChoices; label: MessageId }> = [
	{ key: 'places', label: 'destination.section.places' },
	{ key: 'favourites', label: 'destination.section.favourites' },
	{ key: 'tabs', label: 'destination.section.tabs' },
	{ key: 'recent', label: 'destination.section.recent' },
];

function baseName(display: string): string {
	const parts = display.split('/').filter(Boolean);
	return parts[parts.length - 1] ?? display;
}

/**
 * The path field has focus when it opens and Enter in it chooses the folder once it checks out;
 * Esc cancels. Typing is checked a moment after it pauses, and picking a folder from a list fills
 * the field and checks it at once, so what the field says is always what Copy or Move will use.
 * The primary button waits for a folder that exists, is a folder and can be written to, and the
 * line under the field says why not in words until it is.
 */
export function DestinationDialog({
	options,
	vfs,
	choices,
	createFolder,
	debounceMs = 200,
	onConfirm,
	onCancel,
}: DestinationDialogProps) {
	const start = options.initial ?? options.base;
	const [text, setText] = useState(options.startEmpty ? '' : start.display);
	const [status, setStatus] = useState<Status>({ state: 'checking' });
	const statusId = useId();
	const field = useRef<HTMLInputElement | null>(null);
	// A reply that arrives after newer text, or after the dialog closed, must do nothing.
	const attempt = useRef(0);

	const check = (value: string): Promise<DestinationCheck> =>
		checkDestination(vfs, value, options.base, {
			origin: options.origin,
			forbidOrigin: options.forbidOrigin,
		});

	useEffect(() => {
		const mine = ++attempt.current;
		setStatus({ state: 'checking' });
		const timer = setTimeout(
			() => {
				void check(text).then((result) => {
					if (mine === attempt.current) setStatus(result);
				});
			},
			// The first check, and one for a chosen folder, need no pause.
			attempt.current === 1 ? 0 : debounceMs,
		);
		return () => clearTimeout(timer);
		// `check` closes over props that do not change while the dialog is open.
	}, [text]);
	useEffect(
		() => () => {
			attempt.current += 1;
		},
		[],
	);
	useEffect(() => field.current?.select(), []);

	const ok = status.state === 'ok' || status.state === 'made';
	const working = status.state === 'working';

	/** Confirms what the field says now: a check still pending is finished first. */
	const confirm = async () => {
		if (working) return;
		const mine = ++attempt.current;
		const result = status.state === 'ok' ? status : await check(text);
		if (mine !== attempt.current) return;
		if (result.state === 'ok') onConfirm(result.location);
		else setStatus(result);
	};

	const choose = (choice: DestinationChoice) => {
		setText(choice.location.display);
		field.current?.focus();
	};

	const makeFolder = async () => {
		if (!createFolder || status.state !== 'ok') return;
		const parent = status.location;
		const mine = ++attempt.current;
		setStatus({ state: 'working' });
		try {
			const made = await createFolder(parent);
			if (mine !== attempt.current) return;
			// Selecting the new folder is putting its path in the field, checked and ready.
			attempt.current += 1;
			setText(made.display);
			setStatus({ state: 'made', name: baseName(made.display) });
			void check(made.display).then((result) => {
				if (result.state === 'ok') setStatus({ state: 'ok', location: result.location });
				else setStatus(result);
			});
		} catch (error) {
			if (mine !== attempt.current) return;
			const reason = error instanceof Error ? error.message : String(error);
			setStatus({ state: 'problem', message: tf('destination.newFolder.failed', { reason }) });
		}
	};

	const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
		if (event.key !== 'Enter' || event.nativeEvent.isComposing) return;
		event.preventDefault();
		void confirm();
	};

	const message =
		status.state === 'checking'
			? t('destination.check.working')
			: status.state === 'working'
				? t('destination.newFolder.working')
				: status.state === 'made'
					? tf('destination.newFolder.made', { name: status.name })
					: status.state === 'ok'
						? tf('destination.check.ok', {
								name: baseName(status.location.display) || status.location.display,
							})
						: status.message;
	// An empty field is not yet wrong, only unfinished: its line says what to do.
	const problem = status.state === 'problem' && text.trim() !== '';

	return (
		<Dialog
			open
			size="medium"
			title={options.title}
			description={t('destination.path.description')}
			initialFocus="[data-destination-path]"
			onClose={onCancel}
			footer={
				<DialogActions>
					{createFolder && (
						<DialogButton
							className={styles.newFolder}
							disabled={status.state !== 'ok'}
							onClick={() => void makeFolder()}
						>
							{t('destination.newFolder')}
						</DialogButton>
					)}
					<DialogButton onClick={onCancel}>{t('destination.cancel')}</DialogButton>
					<DialogButton variant="primary" disabled={!ok || working} onClick={() => void confirm()}>
						{options.confirmLabel}
					</DialogButton>
				</DialogActions>
			}
		>
			<div className={styles.body}>
				<label className={styles.field}>
					<span className={styles.label}>{t('destination.path.label')}</span>
					<input
						ref={field}
						data-destination-path=""
						className={styles.input}
						type="text"
						spellCheck={false}
						autoComplete="off"
						value={text}
						aria-invalid={problem ? true : undefined}
						aria-describedby={statusId}
						onChange={(event) => setText(event.target.value)}
						onKeyDown={onKeyDown}
					/>
				</label>
				<p
					id={statusId}
					className={styles.status}
					data-state={status.state}
					role={problem ? 'alert' : 'status'}
				>
					{message}
				</p>
				<div className={styles.choices} role="group" aria-label={t('destination.choices.label')}>
					{SECTIONS.filter(({ key }) => choices[key].length > 0).map(({ key, label }) => (
						<section key={key} className={styles.section} aria-label={t(label)}>
							<h3 className={styles.heading}>{t(label)}</h3>
							<ul className={styles.list}>
								{choices[key].map((choice) => (
									<li key={choice.location.uri}>
										<button
											type="button"
											className={styles.choice}
											title={choice.location.display}
											onClick={() => choose(choice)}
										>
											<span className={styles.choiceLabel}>{choice.label}</span>
											<span className={styles.choicePath}>{choice.location.display}</span>
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
