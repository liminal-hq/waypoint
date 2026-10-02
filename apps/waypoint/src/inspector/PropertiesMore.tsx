// The Properties window's extra detail: the permissions taken apart, every time in full, exact sizes, flags and where a link leads
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { EntryDetails } from '@liminal-hq/waypoint-protocol/generated/EntryDetails';
import { useId, type ReactNode } from 'react';
import { useHourCycle } from '../browse/TimeFormatContext';
import { formatSize } from '../browse/format';
import { t, tf } from '../i18n/messages';
import { kindMessage } from './inspectorModel';
import {
	formatFullTime,
	formatIsoTime,
	octalMode,
	permissionRows,
	specialBits,
	type PermissionWho,
	type SpecialBit,
} from './propertiesDetailModel';
import { permissionString } from './inspectorModel';
import styles from './PropertiesMore.module.css';

const WHO_MESSAGE = {
	owner: 'properties.perm.who.owner',
	group: 'properties.perm.who.group',
	others: 'properties.perm.who.others',
} as const satisfies Record<PermissionWho, string>;

const SPECIAL_MESSAGE = {
	setuid: 'properties.special.setuid',
	setgid: 'properties.special.setgid',
	sticky: 'properties.special.sticky',
} as const satisfies Record<SpecialBit, string>;

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

const yesNo = (value: boolean) => t(value ? 'properties.yes' : 'properties.no');

/** A field the provider cannot report at all reads "Unavailable"; one the entry does not have is left out. */
function unavailable() {
	return <span className={styles.unavailable}>{t('inspector.unavailable')}</span>;
}

/**
 * What the narrow panel leaves out. It shows only what the details response holds: a provider
 * that cannot report a field says "Unavailable", and one the entry does not have is left out.
 */
export function PropertiesMore({ entry, details }: { entry: Entry; details: EntryDetails }) {
	const hourCycle = useHourCycle();
	const heading = useId();
	const time = (ms: number | null, missing: boolean) => {
		if (missing) return unavailable();
		if (ms === null) return null;
		return (
			<time dateTime={formatIsoTime(ms)} className={styles.time}>
				{formatFullTime(ms, hourCycle)}
				<span className={styles.detail}>
					{tf('properties.times.utc', { time: formatIsoTime(ms) })}
				</span>
			</time>
		);
	};
	const missing = (field: EntryDetails['unavailable'][number]) =>
		details.unavailable.includes(field);
	const created = time(details.createdMs, missing('created'));
	const modified = time(details.modifiedMs ?? entry.modifiedMs, false);
	const accessed = time(details.accessedMs, missing('accessed'));
	const mode = details.mode;
	const special = mode === null ? [] : specialBits(mode);
	const size = details.size ?? entry.size;

	return (
		<section className={styles.section} aria-labelledby={heading}>
			<h2 id={heading} className={styles.heading}>
				{t('properties.more.title')}
			</h2>
			<dl className={styles.list}>
				{modified && <Row label={t('inspector.field.modified')}>{modified}</Row>}
				{created && <Row label={t('inspector.field.created')}>{created}</Row>}
				{accessed && <Row label={t('inspector.field.accessed')}>{accessed}</Row>}
				{size !== null && details.kind !== 'directory' && (
					<Row label={t('properties.field.exactSize')}>
						{tf('inspector.size.bytes', { bytes: new Intl.NumberFormat().format(size) })}
						<span className={styles.detail}>{formatSize(size)}</span>
					</Row>
				)}
				{details.allocatedSize !== null && !missing('allocatedSize') && (
					<Row label={t('properties.field.allocated')}>
						{tf('inspector.size.bytes', {
							bytes: new Intl.NumberFormat().format(details.allocatedSize),
						})}
					</Row>
				)}
				{missing('owner') ? (
					<Row label={t('properties.field.owner')}>{unavailable()}</Row>
				) : (
					details.owner !== null && <Row label={t('properties.field.owner')}>{details.owner}</Row>
				)}
				{missing('group') ? (
					<Row label={t('properties.field.group')}>{unavailable()}</Row>
				) : (
					details.group !== null && <Row label={t('properties.field.group')}>{details.group}</Row>
				)}
				{missing('permissions') ? (
					<Row label={t('inspector.field.permissions')}>{unavailable()}</Row>
				) : (
					mode !== null && (
						<>
							<Row label={t('properties.field.mode')}>
								<span className={styles.mono}>{permissionString(mode)}</span>
								<span className={styles.detail}>{octalMode(mode)}</span>
							</Row>
							{special.length > 0 && (
								<Row label={t('properties.field.special')}>
									{special.map((bit) => t(SPECIAL_MESSAGE[bit])).join(', ')}
								</Row>
							)}
						</>
					)
				)}
				<Row label={t('properties.field.readOnly')}>{yesNo(details.readOnly)}</Row>
				<Row label={t('properties.field.hidden')}>{yesNo(details.hidden)}</Row>
				{details.kind === 'symlink' && (
					<>
						{details.symlinkTarget !== null && (
							<Row label={t('inspector.field.linkTarget')}>{details.symlinkTarget}</Row>
						)}
						<Row label={t('properties.field.leadsTo')}>
							{details.resolvesTo === null
								? t('inspector.link.broken')
								: t(kindMessage(details.resolvesTo, entry.group))}
						</Row>
					</>
				)}
			</dl>
			{mode !== null && !missing('permissions') && (
				<table className={styles.table}>
					<caption className={styles.caption}>{t('properties.perm.caption')}</caption>
					<thead>
						<tr>
							<th scope="col">{t('properties.perm.who')}</th>
							<th scope="col">{t('properties.perm.read')}</th>
							<th scope="col">{t('properties.perm.write')}</th>
							<th scope="col">{t('properties.perm.execute')}</th>
						</tr>
					</thead>
					<tbody>
						{permissionRows(mode).map((row) => (
							<tr key={row.who}>
								<th scope="row">{t(WHO_MESSAGE[row.who])}</th>
								<td>{yesNo(row.read)}</td>
								<td>{yesNo(row.write)}</td>
								<td>{yesNo(row.execute)}</td>
							</tr>
						))}
					</tbody>
				</table>
			)}
		</section>
	);
}
