// The Properties window: the Inspector's Properties content for one entry, extra detail, and a checksum on request
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { tauriWindowControls } from '@liminal-hq/waypoint-chrome/TitleBar/tauriWindowControls';
import { WindowFrame } from '@liminal-hq/waypoint-chrome/WindowFrame';
import { useEffect, useMemo, useState, type ReactNode } from 'react';
import { FileIcon } from '../browse/FileIcon';
import { TimeFormatProvider } from '../browse/TimeFormatContext';
import { VfsClientProvider } from '../browse/VfsClientContext';
import { t, tf } from '../i18n/messages';
import { ChecksumSection } from '../inspector/ChecksumSection';
import { DetailsClientProvider } from '../inspector/DetailsClientContext';
import { baseName, kindMessage } from '../inspector/inspectorModel';
import { PropertiesMore } from '../inspector/PropertiesMore';
import { PropertiesPane } from '../inspector/PropertiesPane';
import { useEntryDetails } from '../inspector/useEntryDetails';
import {
	usePropertiesWindowSubject,
	type WindowSubject,
} from '../inspector/usePropertiesWindowSubject';
import { createTauriOpenWithClient } from '../openWith/tauriOpenWithClient';
import { OpenWithProvider } from '../openWith/OpenWithContext';
import type { OpenWithClient } from '../openWith/openWithClient';
import { useFreeSpace } from '../status/useFreeSpace';
import { formatSize } from '../browse/format';
import type { ChecksumClient } from '../services/checksumClient';
import type { DetailsClient } from '../services/detailsClient';
import type { PropertiesWindowClient } from '../services/propertiesWindowClient';
import { createTauriChecksumClient } from '../services/tauriChecksumClient';
import { createTauriDetailsClient } from '../services/tauriDetailsClient';
import { createTauriPropertiesWindowClient } from '../services/tauriPropertiesWindowClient';
import { createTauriTimeFormatClient } from '../services/tauriTimeFormatClient';
import { createTauriVfsClient } from '../services/tauriVfsClient';
import type { TimeFormatClient } from '../services/timeFormatClient';
import type { VfsClient } from '../services/vfsClient';
import { AppTitleBar } from './AppTitleBar';
import { NoticeToast } from './NoticeToast';
import styles from './PropertiesScreen.module.css';

interface PropertiesScreenProps {
	/** The services the window reads; the real ones unless a test supplies its own. */
	vfs?: VfsClient;
	details?: DetailsClient;
	windows?: PropertiesWindowClient;
	checksum?: ChecksumClient;
	timeFormat?: TimeFormatClient;
	openWith?: OpenWithClient;
	/** Closes the window (Esc, and the Close button); the real window's unless a test supplies its own. */
	onClose?: () => void;
	/** Puts text on the clipboard; the browser's unless a test supplies its own. */
	writeText?: (text: string) => Promise<void>;
}

/** A root folder has no folder to be listed in, so what is known of it is its name, place and space. */
function RootBody({
	vfs,
	location,
}: {
	vfs: VfsClient;
	location: { display: string; uri: string };
}) {
	const space = useFreeSpace(vfs, location, 0);
	return (
		<dl className={styles.facts}>
			<div className={styles.fact}>
				<dt>{t('inspector.field.name')}</dt>
				<dd data-selectable="">{baseName(location.display)}</dd>
			</div>
			<div className={styles.fact}>
				<dt>{t('inspector.field.kind')}</dt>
				<dd>{t('inspector.kind.folder')}</dd>
			</div>
			<div className={styles.fact}>
				<dt>{t('inspector.field.location')}</dt>
				<dd data-selectable="">{location.display}</dd>
			</div>
			{space && (
				<div className={styles.fact}>
					<dt>{t('inspector.field.freeSpace')}</dt>
					<dd>
						{tf('inspector.freeOf', {
							free: formatSize(space.freeBytes),
							total: formatSize(space.totalBytes),
						})}
					</dd>
				</div>
			)}
		</dl>
	);
}

function EntryBody({
	subject,
	details,
	checksum,
	writeText,
}: {
	subject: Extract<WindowSubject, { status: 'entry' }>;
	details: DetailsClient;
	checksum: ChecksumClient;
	writeText?: (text: string) => Promise<void>;
}) {
	const { entry, handle, parent } = subject;
	const state = useEntryDetails(details, handle, entry);
	const read = state.status === 'ready' ? state.details : null;
	return (
		<>
			<PropertiesPane
				subject={{ kind: 'entry', entry, handle }}
				session={null}
				client={details}
				details={state}
				folder={parent}
				active
			/>
			{read && <PropertiesMore entry={entry} details={read} />}
			<ChecksumSection
				client={checksum}
				handle={handle}
				entry={entry}
				details={read}
				{...(writeText ? { writeText } : {})}
			/>
		</>
	);
}

function Body({
	vfs,
	details,
	windows,
	checksum,
	onClose,
	writeText,
}: {
	vfs: VfsClient;
	details: DetailsClient;
	windows: PropertiesWindowClient;
	checksum: ChecksumClient;
	onClose: () => void;
	writeText?: (text: string) => Promise<void>;
}) {
	const subject = usePropertiesWindowSubject(vfs, windows);

	// Esc closes the window. A key something else took (a toast, a select's list) is left alone.
	useEffect(() => {
		const onKeyDown = (event: KeyboardEvent) => {
			if (event.key !== 'Escape' || event.defaultPrevented || event.isComposing) return;
			event.preventDefault();
			onClose();
		};
		window.addEventListener('keydown', onKeyDown);
		return () => window.removeEventListener('keydown', onKeyDown);
	}, [onClose]);

	const name =
		subject.status === 'entry'
			? subject.entry.name
			: subject.status === 'gone'
				? subject.name
				: subject.status === 'root'
					? baseName(subject.location.display)
					: '';
	useEffect(() => {
		document.title = name ? tf('properties.window.label', { name }) : t('window.properties.title');
	}, [name]);

	let content: ReactNode;
	let icon: ReactNode = null;
	switch (subject.status) {
		case 'loading':
			content = <p className={styles.note}>{t('properties.window.loading')}</p>;
			break;
		case 'failed':
			content = <p className={styles.note}>{t('properties.window.failed')}</p>;
			break;
		case 'gone':
			icon = <FileIcon group="other" className={styles.icon} />;
			content = <p className={styles.note}>{t('properties.window.gone.note')}</p>;
			break;
		case 'root':
			icon = <FileIcon group="folder" className={styles.icon} />;
			content = <RootBody vfs={vfs} location={subject.location} />;
			break;
		case 'entry':
			icon = <FileIcon group={subject.entry.group} className={styles.icon} />;
			content = (
				<EntryBody
					subject={subject}
					details={details}
					checksum={checksum}
					{...(writeText ? { writeText } : {})}
				/>
			);
			break;
	}

	return (
		<main
			className={styles.content}
			aria-label={name ? tf('properties.window.label', { name }) : t('window.properties.title')}
			data-status={subject.status}
		>
			<header className={styles.header}>
				{icon}
				<div className={styles.headerText}>
					<h1 className={styles.title} data-selectable="">
						{subject.status === 'gone'
							? tf('properties.window.gone.title', { name })
							: name || t('window.properties.title')}
					</h1>
					{subject.status === 'entry' && (
						<p className={styles.kind}>{t(kindMessage(subject.entry.kind, subject.entry.group))}</p>
					)}
				</div>
			</header>
			<div className={styles.body}>{content}</div>
		</main>
	);
}

/**
 * The standalone Properties window (`properties-{n}`, made by the app's factory). It asks Rust what
 * it is about, lists that entry's folder and follows the entry through renames; the Inspector's
 * Properties content is reused as it is, with the extra detail the narrow panel leaves out and a
 * checksum that is calculated only on request.
 */
export function PropertiesScreen({
	vfs,
	details,
	windows,
	checksum,
	timeFormat,
	openWith,
	onClose,
	writeText,
}: PropertiesScreenProps) {
	const [ownVfs] = useState(() => vfs ?? createTauriVfsClient());
	const [ownDetails] = useState(() => details ?? createTauriDetailsClient());
	const [ownWindows] = useState(() => windows ?? createTauriPropertiesWindowClient());
	const [ownChecksum] = useState(() => checksum ?? createTauriChecksumClient());
	const [ownTime] = useState(() => timeFormat ?? createTauriTimeFormatClient());
	const [ownOpenWith] = useState(() => openWith ?? createTauriOpenWithClient());
	const close = useMemo(() => onClose ?? (() => void tauriWindowControls.close()), [onClose]);
	return (
		<WindowFrame className={styles.screen}>
			<AppTitleBar title={t('window.properties.title')} />
			<VfsClientProvider client={ownVfs}>
				<TimeFormatProvider client={ownTime}>
					<OpenWithProvider client={ownOpenWith}>
						<DetailsClientProvider client={ownDetails}>
							<Body
								vfs={ownVfs}
								details={ownDetails}
								windows={ownWindows}
								checksum={ownChecksum}
								onClose={close}
								{...(writeText ? { writeText } : {})}
							/>
						</DetailsClientProvider>
					</OpenWithProvider>
				</TimeFormatProvider>
			</VfsClientProvider>
			<NoticeToast />
		</WindowFrame>
	);
}
