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
	useRef,
	useState,
	type FocusEvent,
	type FormEvent,
	type KeyboardEvent,
} from 'react';
import { useVfsClient } from '../browse/VfsClientContext';
import { ChevronRightSmallIcon } from '../icons/AppIcons';
import { t, tf } from '../i18n/messages';
import { toVfsError } from '../browse/listingModel';
import { useLocationInfo } from './locationInfo';
import styles from './PathBar.module.css';

interface PathBarProps {
	location: Location;
	editing: boolean;
	onEditingChange: (editing: boolean) => void;
	onNavigate: (location: Location) => void;
}

function problemText(error: VfsError, input: string): string {
	switch (error.kind) {
		case 'invalidLocation':
			return tf('nav.path.invalid', { input });
		case 'unsupported':
			return tf('nav.path.unsupported', { what: error.what });
		default:
			return t('nav.path.failed');
	}
}

export function PathBar({ location, editing, onEditingChange, onNavigate }: PathBarProps) {
	return editing ? (
		<PathEditor
			location={location}
			onClose={() => onEditingChange(false)}
			onNavigate={onNavigate}
		/>
	) : (
		<Breadcrumbs location={location} onEdit={() => onEditingChange(true)} onNavigate={onNavigate} />
	);
}

interface BreadcrumbsProps {
	location: Location;
	onEdit: () => void;
	onNavigate: (location: Location) => void;
}

function Breadcrumbs({ location, onEdit, onNavigate }: BreadcrumbsProps) {
	const client = useVfsClient();
	const info = useLocationInfo(client, location, { keepPrevious: true });
	const scroller = useRef<HTMLOListElement | null>(null);

	// A long path shows its tail: the folder you are in matters more than the root.
	useLayoutEffect(() => {
		const element = scroller.current;
		if (element) element.scrollLeft = element.scrollWidth;
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
									aria-current={last ? 'page' : undefined}
									onClick={() => onNavigate(segment.location)}
								>
									{segment.label}
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
	onNavigate: (location: Location) => void;
}

function PathEditor({ location, onClose, onNavigate }: PathEditorProps) {
	const client = useVfsClient();
	const field = useRef<HTMLInputElement | null>(null);
	const [text, setText] = useState(location.display);
	const [problem, setProblem] = useState<string | null>(null);
	const problemId = useId();
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
			const target = await client.parseLocation(text, location);
			if (mine !== attempt.current) return;
			if (target.uri !== location.uri) onNavigate(target);
			onClose();
		} catch (error) {
			if (mine !== attempt.current) return;
			setProblem(problemText(toVfsError(error), text));
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
				className={styles.input}
				type="text"
				spellCheck={false}
				autoComplete="off"
				aria-label={t('nav.path.input')}
				aria-invalid={problem ? true : undefined}
				aria-describedby={problem ? problemId : undefined}
				value={text}
				onChange={(event) => {
					setText(event.target.value);
					setProblem(null);
				}}
				onKeyDown={onKeyDown}
			/>
			{problem && (
				<p id={problemId} className={styles.problem} role="alert">
					{problem}
				</p>
			)}
		</form>
	);
}
