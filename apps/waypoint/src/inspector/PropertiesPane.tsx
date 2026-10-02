// The Properties tab: the facts about the selection, the folder size as it is counted, and what an unreadable field says
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { DetailField } from '@liminal-hq/waypoint-protocol/generated/DetailField';
import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { EntryDetails } from '@liminal-hq/waypoint-protocol/generated/EntryDetails';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { VolumeSpace } from '@liminal-hq/waypoint-protocol/generated/VolumeSpace';
import type { ReactNode } from 'react';
import { formatModified, formatSize } from '../browse/format';
import { useHourCycle } from '../browse/TimeFormatContext';
import type { ListingSession } from '../browse/useListingSession';
import { useOptionalVfsClient } from '../browse/VfsClientContext';
import { t, tf, tn } from '../i18n/messages';
import { useFileCommands } from '../ops/FileCommandsContext';
import type { DetailsClient } from '../services/detailsClient';
import type { VfsClient } from '../services/vfsClient';
import { useSelectionSummary } from '../status/useSelectionSummary';
import { useFreeSpace } from '../status/useFreeSpace';
import { baseName, formatPermissions, isFolderKind, kindMessage } from './inspectorModel';
import type { InspectorSubject } from './useInspectorSubject';
import type { DetailsState } from './useEntryDetails';
import { useEntryExtras } from './useEntryExtras';
import { useFolderSize, type FolderSizeState } from './useFolderSize';
import { useSettled } from './useSettled';
import { HEAVY_DELAY_MS } from './inspectorModel';
import styles from './PropertiesPane.module.css';

interface PropertiesPaneProps {
	subject: InspectorSubject;
	session: ListingSession | null;
	client: DetailsClient | null;
	/** What the panel has read of the selected entry. */
	details: DetailsState;
	/** The folder the listing shows, which is where the selected entry is. */
	folder: Location | undefined;
	/** The pane is showing: nothing heavy runs while it is not. */
	active: boolean;
}

/** One labelled fact; `value` is what is read out after the label. */
function Row({ label, children }: { label: string; children: ReactNode }) {
	return (
		<div className={styles.row}>
			<dt className={styles.label}>{label}</dt>
			<dd className={styles.value} data-selectable="">
				{children}
			</dd>
		</div>
	);
}

/** The Properties tab for whatever the Inspector is about. */
export function PropertiesPane({
	subject,
	session,
	client,
	details,
	folder,
	active,
}: PropertiesPaneProps) {
	switch (subject.kind) {
		case 'none':
			return <p className={styles.empty}>{t('inspector.properties.empty')}</p>;
		case 'folder':
			return (
				<FolderProperties location={subject.location} count={subject.count} session={session} />
			);
		case 'many':
			return <ManyProperties session={session} />;
		case 'entry':
			return (
				<EntryProperties
					entry={subject.entry}
					handle={subject.handle}
					session={session}
					client={client}
					state={details}
					parent={folder}
					active={active}
				/>
			);
	}
}

/** Nothing selected: the folder being shown, with what the listing and the volume say of it. */
function FolderProperties({
	location,
	count,
	session,
}: {
	location: Location;
	count: number;
	session: ListingSession | null;
}) {
	const vfs = useOptionalVfsClient();
	return (
		<dl className={styles.list}>
			<Row label={t('inspector.field.name')}>{baseName(location.display)}</Row>
			<Row label={t('inspector.field.kind')}>{t('inspector.kind.folder')}</Row>
			<Row label={t('inspector.field.location')}>{location.display}</Row>
			<Row label={t('inspector.field.contains')}>{tn('status.items', count)}</Row>
			{vfs && <FolderSpace client={vfs} location={location} session={session} />}
		</dl>
	);
}

/** The volume's free space, read again as the folder changes (throttled, as the status bar's is). */
function FolderSpace({
	client,
	location,
	session,
}: {
	client: VfsClient;
	location: Location;
	session: ListingSession | null;
}) {
	const space = useFreeSpace(client, location, session?.model.revision ?? 0);
	return space ? <Row label={t('inspector.field.freeSpace')}>{freeOf(space)}</Row> : null;
}

/** Several selected: how many, and the size of the files among them (folders are not walked). */
function ManyProperties({ session }: { session: ListingSession | null }) {
	const vfs = useOptionalVfsClient();
	return (
		<>
			{vfs && session ? (
				<ManySummary client={vfs} session={session} />
			) : (
				<p className={styles.empty}>{t('inspector.properties.empty')}</p>
			)}
			<p className={styles.note}>{t('inspector.many.note')}</p>
		</>
	);
}

function ManySummary({ client, session }: { client: VfsClient; session: ListingSession }) {
	const summary = useSelectionSummary(client, session);
	return (
		<dl className={styles.list}>
			<Row label={t('inspector.field.selected')}>{tn('browse.selection', summary.count)}</Row>
			{summary.size !== null && (
				<Row label={t('inspector.field.totalSize')}>
					<span data-pending={summary.pending ? '' : undefined}>{formatSize(summary.size)}</span>
				</Row>
			)}
		</dl>
	);
}

function freeOf(space: VolumeSpace): string {
	return tf('inspector.freeOf', {
		free: formatSize(space.freeBytes),
		total: formatSize(space.totalBytes),
	});
}

/** The row for a field the provider cannot report at all, or `null` for one this entry does not have. */
function field(
	details: EntryDetails,
	which: DetailField,
	label: string,
	value: ReactNode | null,
): ReactNode {
	if (details.unavailable.includes(which)) {
		return (
			<Row label={label}>
				<span className={styles.unavailable}>{t('inspector.unavailable')}</span>
			</Row>
		);
	}
	return value === null ? null : <Row label={label}>{value}</Row>;
}

function sizeText(state: FolderSizeState): string {
	switch (state.status) {
		case 'done':
			return formatSize(state.totals.bytes);
		case 'failed':
			return t('inspector.unavailable');
		case 'calculating':
			return state.totals
				? tf('inspector.size.calculatingSoFar', { size: formatSize(state.totals.bytes) })
				: t('inspector.size.calculating');
		case 'idle':
			return t('inspector.size.calculating');
	}
}

function EntryProperties({
	entry,
	handle,
	session,
	client,
	state,
	parent,
	active,
}: {
	entry: Entry;
	handle: ListingHandle;
	session: ListingSession | null;
	client: DetailsClient | null;
	state: DetailsState;
	parent: Location | undefined;
	active: boolean;
}) {
	const hourCycle = useHourCycle();
	const commands = useFileCommands();
	const details = state.status === 'ready' ? state.details : null;
	const folder = isFolderKind(entry.kind, details?.resolvesTo ?? entry.linkTarget);
	const key = `${handle}:${entry.id}`;
	const settled = useSettled(key, HEAVY_DELAY_MS) && active;
	const sizeRun = useFolderSize(client, handle, folder ? entry.id : null, active);
	const extras = useEntryExtras(handle, entry.id, folder, settled);
	const date = (ms: number | null) =>
		ms === null ? null : formatModified(ms, undefined, hourCycle);
	const canRename = commands?.states(session).rename?.enabled === true;

	// Only a finished count is announced: progress would talk over the person's own keystrokes.
	const announcement =
		sizeRun.status === 'done'
			? tf('inspector.size.announce', { name: entry.name, size: formatSize(sizeRun.totals.bytes) })
			: '';
	const modified = details?.modifiedMs ?? entry.modifiedMs;

	return (
		<>
			<dl className={styles.list}>
				<Row label={t('inspector.field.name')}>
					<span className={styles.nameValue}>
						<span>{entry.name}</span>
						{canRename && (
							<button
								type="button"
								className={styles.button}
								onClick={() => commands?.rename(session, entry)}
							>
								{t('inspector.rename')}
							</button>
						)}
					</span>
				</Row>
				<Row label={t('inspector.field.kind')}>{t(kindMessage(entry.kind, entry.group))}</Row>
				{folder
					? client && (
							<Row label={t('inspector.field.size')}>
								<span data-calculating={sizeRun.status === 'calculating' ? '' : undefined}>
									{sizeText(sizeRun)}
								</span>
								{sizeRun.status === 'done' && (
									<span className={styles.detail}>
										{tf('inspector.size.contents', {
											files: tn('inspector.files', sizeRun.totals.files),
											folders: tn('inspector.folders', sizeRun.totals.folders),
										})}
									</span>
								)}
							</Row>
						)
					: (details?.size ?? entry.size) !== null && (
							<Row label={t('inspector.field.size')}>
								{formatSize(details?.size ?? entry.size ?? 0)}
								<span className={styles.detail}>
									{tf('inspector.size.bytes', {
										bytes: new Intl.NumberFormat().format(details?.size ?? entry.size ?? 0),
									})}
								</span>
							</Row>
						)}
				{details && !folder
					? field(
							details,
							'allocatedSize',
							t('inspector.field.onDisk'),
							details.allocatedSize === null ? null : formatSize(details.allocatedSize),
						)
					: null}
				<Row label={t('inspector.field.location')}>
					{entry.originalPath ?? parent?.display ?? ''}
				</Row>
				{modified !== null && <Row label={t('inspector.field.modified')}>{date(modified)}</Row>}
				{details && (
					<>
						{field(details, 'created', t('inspector.field.created'), date(details.createdMs))}
						{field(details, 'accessed', t('inspector.field.accessed'), date(details.accessedMs))}
						{field(details, 'owner', t('inspector.field.owner'), details.owner)}
						{field(details, 'group', t('inspector.field.group'), details.group)}
						{field(
							details,
							'permissions',
							t('inspector.field.permissions'),
							details.mode === null ? null : formatPermissions(details.mode),
						)}
						{details.symlinkTarget !== null ? (
							<Row label={t('inspector.field.linkTarget')}>{details.symlinkTarget}</Row>
						) : details.kind === 'symlink' ? (
							<Row label={t('inspector.field.linkTarget')}>{t('inspector.link.broken')}</Row>
						) : null}
						{details.mimeType !== null && (
							<Row label={t('inspector.field.contentType')}>{details.mimeType}</Row>
						)}
					</>
				)}
				{folder && extras.space && (
					<Row label={t('inspector.field.freeSpace')}>
						{tf('inspector.freeOf', {
							free: formatSize(extras.space.freeBytes),
							total: formatSize(extras.space.totalBytes),
						})}
					</Row>
				)}
				{!folder && extras.defaultApp !== undefined && (
					<Row label={t('inspector.field.defaultApp')}>
						{extras.defaultApp ?? t('inspector.defaultApp.none')}
					</Row>
				)}
			</dl>
			{state.status === 'failed' && <p className={styles.note}>{t('inspector.details.failed')}</p>}
			<div className={styles.srOnly} role="status" aria-live="polite">
				{announcement}
			</div>
		</>
	);
}
