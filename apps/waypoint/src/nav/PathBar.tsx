// The path bar: clickable breadcrumbs that become a text field, for typing a location to go to
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import {
	useEffect,
	useId,
	useLayoutEffect,
	useMemo,
	useRef,
	useState,
	type FocusEvent,
	type FormEvent,
	type KeyboardEvent,
} from 'react';
import { useConnectionsView } from '../connections/ConnectionsContext';
import { schemeLabel } from '../connections/connectModel';
import { ExperimentalLink } from '../connections/ProtocolOff';
import { useVfsClient } from '../browse/VfsClientContext';
import { dropAttributes } from '../dnd/dropTargets';
import { ChevronRightSmallIcon } from '../icons/AppIcons';
import { endScrollLeft, isRtl } from '../i18n/direction';
import { Own } from '../i18n/bidi';
import { t, tf } from '../i18n/messages';
import { toVfsError } from '../browse/listingModel';
import { useLocationInfo } from './locationInfo';
import styles from './PathBar.module.css';

interface PathBarProps {
	location: Location;
	editing: boolean;
	onEditingChange: (editing: boolean) => void;
	onNavigate: (location: Location) => void | Promise<void>;
	/** Middle-click on a path part: it opens in a tab beside this one, in the background, or in a new window with Ctrl. */
	onOpenInNewTab?: (location: Location, inNewWindow: boolean) => void;
	/** The typed path was accepted (whether or not it went anywhere), so focus can move on to the list. */
	onCommitted?: () => void;
}

function problemText(error: VfsError, input: string): string {
	switch (error.kind) {
		case 'invalidLocation':
			return tf('nav.path.invalid', { input });
		case 'unsupported':
			return tf('nav.path.unsupported', { what: error.what });
		case 'protocolOff':
			return tf('nav.path.protocolOff', { protocol: schemeLabel(error.scheme) });
		default:
			return t('nav.path.failed');
	}
}

export function PathBar({
	location,
	editing,
	onEditingChange,
	onNavigate,
	onOpenInNewTab,
	onCommitted,
}: PathBarProps) {
	return editing ? (
		<PathEditor
			location={location}
			onClose={() => onEditingChange(false)}
			onNavigate={onNavigate}
			onCommitted={onCommitted}
		/>
	) : (
		<Breadcrumbs
			location={location}
			onEdit={() => onEditingChange(true)}
			onNavigate={onNavigate}
			onOpenInNewTab={onOpenInNewTab}
		/>
	);
}

interface BreadcrumbsProps {
	location: Location;
	onEdit: () => void;
	onNavigate: (location: Location) => void;
	onOpenInNewTab?: (location: Location, inNewWindow: boolean) => void;
}

function Breadcrumbs({ location, onEdit, onNavigate, onOpenInNewTab }: BreadcrumbsProps) {
	const client = useVfsClient();
	const info = useLocationInfo(client, location, { keepPrevious: true });
	const scroller = useRef<HTMLOListElement | null>(null);

	// A long path shows its tail: the folder you are in matters more than the root.
	useLayoutEffect(() => {
		const element = scroller.current;
		if (element) element.scrollLeft = endScrollLeft(element.scrollWidth, isRtl(element));
	}, [info]);

	const segments = info?.segments ?? [];
	return (
		<div className={styles.bar}>
			<nav className={styles.crumbsNav} aria-label={t('nav.path.crumbs')}>
				<ol ref={scroller} className={styles.crumbs}>
					{segments.map((segment, index) => {
						const last = index === segments.length - 1;
						return (
							<li key={segment.location.uri} className={styles.crumbItem}>
								{index > 0 && (
									<ChevronRightSmallIcon className={styles.separator} width={12} height={12} />
								)}
								<button
									type="button"
									className={styles.crumb}
									{...dropAttributes('crumb', segment.location.uri, segment.label)}
									aria-current={last ? 'page' : undefined}
									onClick={() => onNavigate(segment.location)}
									// Stops the middle button starting autoscroll on Linux before `auxclick` arrives.
									onMouseDown={(event) => {
										if (event.button === 1) event.preventDefault();
									}}
									onAuxClick={(event) => {
										if (event.button !== 1 || !onOpenInNewTab) return;
										event.preventDefault();
										onOpenInNewTab(segment.location, event.ctrlKey);
									}}
								>
									<Own>{segment.label}</Own>
								</button>
							</li>
						);
					})}
				</ol>
			</nav>
			<button
				type="button"
				className={styles.editTarget}
				aria-label={t('nav.path.edit')}
				title={t('nav.path.edit')}
				onClick={onEdit}
			/>
		</div>
	);
}

interface PathEditorProps {
	location: Location;
	onClose: () => void;
	onNavigate: (location: Location) => void | Promise<void>;
	onCommitted?: () => void;
}

function PathEditor({ location, onClose, onNavigate, onCommitted }: PathEditorProps) {
	const client = useVfsClient();
	const field = useRef<HTMLInputElement | null>(null);
	const [text, setText] = useState(location.display);
	const [problem, setProblem] = useState<string | null>(null);
	// The address named a protocol that is turned off: the problem links to the page that turns it on.
	const [problemOff, setProblemOff] = useState(false);
	const [note, setNote] = useState<string | null>(null);
	const problemId = useId();
	const noteId = useId();
	const listId = useId();
	// Saved servers and recent ones are offered as the text is typed.
	const saved = useConnectionsView((view) => view.connections);
	const recent = useConnectionsView((view) => view.recent);
	const servers = useMemo(
		() => [
			...new Set([
				...saved.map((entry) => entry.location.display),
				...recent.map((server) => server.location.display),
			]),
		],
		[saved, recent],
	);
	// A reply that arrives after the editor was closed or re-submitted must do nothing.
	const attempt = useRef(0);

	useEffect(() => {
		field.current?.focus();
		field.current?.select();
		return () => {
			attempt.current++;
		};
	}, []);

	const submit = async (event: FormEvent) => {
		event.preventDefault();
		const mine = ++attempt.current;
		try {
			const typed = client.parseLocationText
				? await client.parseLocationText(text, location)
				: { location: await client.parseLocation(text, location), passwordDropped: false };
			if (mine !== attempt.current) return;
			const target = typed.location;
			// A password typed in a server address is never kept (D147): the bar shows the address
			// without it and says so, and the next Enter goes there.
			if (typed.passwordDropped) {
				setText(target.display);
				setNote(t('nav.path.passwordDropped'));
				return;
			}
			onClose();
			if (target.uri !== location.uri) await onNavigate(target);
			onCommitted?.();
		} catch (error) {
			if (mine !== attempt.current) return;
			const failure = toVfsError(error);
			setProblem(problemText(failure, text));
			setProblemOff(failure.kind === 'protocolOff');
		}
	};

	const onKeyDown = (event: KeyboardEvent) => {
		if (event.key === 'Escape') {
			event.preventDefault();
			event.stopPropagation();
			onClose();
		}
	};

	// Clicking away cancels, but moving focus within the bar (none yet) would not.
	const onBlur = (event: FocusEvent<HTMLFormElement>) => {
		if (!event.currentTarget.contains(event.relatedTarget)) onClose();
	};

	return (
		<form className={styles.bar} onSubmit={submit} onBlur={onBlur} noValidate>
			<input
				ref={field}
				// A path or an address is left to right in any layout.
				dir="ltr"
				className={styles.input}
				type="text"
				spellCheck={false}
				autoComplete="off"
				aria-label={t('nav.path.input')}
				aria-invalid={problem ? true : undefined}
				aria-describedby={problem ? problemId : note ? noteId : undefined}
				list={servers.length > 0 ? listId : undefined}
				value={text}
				onChange={(event) => {
					setText(event.target.value);
					setProblem(null);
					setProblemOff(false);
					setNote(null);
				}}
				onKeyDown={onKeyDown}
			/>
			{servers.length > 0 && (
				<datalist id={listId}>
					{servers.map((server) => (
						<option key={server} value={server} />
					))}
				</datalist>
			)}
			{problem && (
				<p id={problemId} className={styles.problem} role="alert">
					{problem}
				</p>
			)}
			{problem && problemOff && <ExperimentalLink />}
			{note && !problem && (
				<p id={noteId} className={styles.problem} role="status" data-tone="note">
					{note}
				</p>
			)}
		</form>
	);
}
