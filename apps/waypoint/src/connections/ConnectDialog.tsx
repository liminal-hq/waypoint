// The Connect dialog: type or fill in a server, test it, save it to the Network section, edit, copy or forget saved ones, and connect
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Dialog } from '@liminal-hq/waypoint-chrome/Dialog/Dialog';
import { DialogActions, DialogButton } from '@liminal-hq/waypoint-chrome/Dialog/DialogActions';
import type { AuthMethod } from '@liminal-hq/waypoint-protocol/generated/AuthMethod';
import type { ConnectionEntry } from '@liminal-hq/waypoint-protocol/generated/ConnectionEntry';
import type { ConnectionSupport } from '@liminal-hq/waypoint-protocol/generated/ConnectionSupport';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { SuggestedServer } from '@liminal-hq/waypoint-protocol/generated/SuggestedServer';
import { useEffect, useId, useRef, useState, type FormEvent, type ReactNode } from 'react';
import { t, tf, type MessageId } from '../i18n/messages';
import { isVfsError } from '../services/vfsClient';
import { connectAnswering, type Answered, type Ask } from './connectFlow';
import type { ConnectionsClient } from './connectionsClient';
import {
	connectionErrorText,
	draftOf,
	draftProblem,
	emptyForm,
	familyOf,
	formOf,
	formProblem,
	keyringText,
	methodsFor,
	rememberedText,
	schemeLabel,
	withScheme,
	type ConnectForm,
	type FormField,
} from './connectModel';
import type { ConnectRequest } from './connectStore';
import { useConnectionsView } from './ConnectionsContext';
import { ExperimentalLink } from './ProtocolOff';
import { S3_PRESETS, s3Preset } from './s3Presets';
import styles from './Connect.module.css';

/** How long typing in the address waits before Rust reads it. */
const PARSE_DELAY_MS = 200;

const METHODS: Record<AuthMethod, { label: MessageId; hint: (family: string) => MessageId }> = {
	auto: {
		label: 'connect.auth.auto',
		hint: (family) =>
			family === 'smb'
				? 'connect.auth.autoHintSmb'
				: family === 'dav'
					? 'connect.auth.autoHintDav'
					: 'connect.auth.autoHint',
	},
	password: { label: 'connect.auth.password', hint: () => 'connect.auth.passwordHint' },
	keyFile: { label: 'connect.auth.keyFile', hint: () => 'connect.auth.keyFileHint' },
	token: { label: 'connect.auth.token', hint: () => 'connect.auth.tokenHint' },
};

/** What the one input under an S3 service asks for, and a line on it. */
const S3_INPUT_LABEL = {
	none: 'connect.field.s3Region',
	region: 'connect.field.s3ServiceRegion',
	accountId: 'connect.field.s3Account',
	host: 'connect.field.s3Host',
	endpoint: 'connect.field.s3Endpoint',
} as const satisfies Record<string, MessageId>;
const S3_INPUT_HINT = {
	none: 'connect.field.s3RegionHint',
	region: 'connect.field.s3ServiceRegionHint',
	accountId: 'connect.field.s3AccountHint',
	host: 'connect.field.s3HostHint',
	endpoint: 'connect.field.s3EndpointHint',
} as const satisfies Record<string, MessageId>;

/** The address field's example, in the form of the protocol chosen. */
const PLACEHOLDERS: Record<string, MessageId> = {
	smb: 'connect.address.placeholderSmb',
	davs: 'connect.address.placeholderDav',
	dav: 'connect.address.placeholderDav',
	s3: 'connect.address.placeholderS3',
};

/** The preview choices a connection offers, in order (D166). */
const THUMBNAIL_CHOICES = ['off', 'smallFiles', 'always'] as const;
export interface ConnectDialogProps {
	client: ConnectionsClient;
	request: ConnectRequest;
	/** The saved connections, in the Network section's order. */
	saved: readonly ConnectionEntry[];
	/** Asks the questions a connection raises (the window's question dialogs). */
	ask: Ask;
	/** Opens a connected server in a new tab. */
	onOpen(location: Location): void;
	/** Says something politely to a screen reader. */
	announce(message: string): void;
	onClose(): void;
}

type Busy = 'test' | 'save' | 'connect' | null;
type Result = { tone: 'ok' | 'failed'; text: string } | null;
type Problems = Partial<Record<FormField, string>>;

/**
 * The address field has focus when the dialog opens. Typing an address (`sftp://me@nas.lan/srv`)
 * fills the fields as Rust reads it, and says when a password written in it was dropped (D147); the
 * fields can also be filled one by one. Test connects without saving and says the outcome in words,
 * keeping every field; Save adds the connection to the Network section (or changes the one being
 * edited); Connect connects and opens the server in a new tab. A login or a host key asked for on
 * the way is asked in a dialog of its own. Nothing here is ever the default for a destructive
 * action: forgetting a saved connection asks first, starting on Cancel.
 */
export function ConnectDialog({
	client,
	request,
	saved,
	ask,
	onOpen,
	announce,
	onClose,
}: ConnectDialogProps) {
	const initial =
		request.mode === 'edit' ? saved.find((e) => e.connection.id === request.id) : null;
	const [editing, setEditing] = useState<string | null>(initial?.connection.id ?? null);
	const [form, setForm] = useState<ConnectForm>(() =>
		initial ? formOf(initial.connection) : emptyForm(),
	);
	const [address, setAddress] = useState(
		initial?.location.display ?? (request.mode === 'new' ? (request.address ?? '') : ''),
	);
	const [addressNote, setAddressNote] = useState('');
	// The address named a protocol that is turned off: the problem links to the page that turns it on.
	const [addressOff, setAddressOff] = useState(false);
	const [problems, setProblems] = useState<Problems>({});
	const [password, setPassword] = useState('');
	// An S3 session token: it expires, so it is kept for this attempt and never remembered.
	const [sessionToken, setSessionToken] = useState('');
	const [remember, setRemember] = useState(false);
	const [busy, setBusy] = useState<Busy>(null);
	const [result, setResult] = useState<Result>(null);
	const [support, setSupport] = useState<ConnectionSupport | null>(null);
	const [suggestions, setSuggestions] = useState<readonly SuggestedServer[]>([]);
	const [forgetting, setForgetting] = useState<ConnectionEntry | null>(null);
	const [forgetLogin, setForgetLogin] = useState(true);
	const parseTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
	const parseTurn = useRef(0);
	// The read of the address that is under way, the address as typed and the form as the newest
	// render or read left it: what a submit settles on, so Enter right after typing uses what the
	// address says.
	const parsing = useRef<Promise<boolean> | null>(null);
	// The control that had focus when an attempt began: disabling it for the attempt drops focus to the page, so it is given back when the attempt ends.
	const focusBack = useRef<HTMLElement | null>(null);
	const formRef = useRef(form);
	formRef.current = form;
	const addressRef = useRef(address);
	addressRef.current = address;
	const ids = {
		address: useId(),
		name: useId(),
		scheme: useId(),
		host: useId(),
		port: useId(),
		user: useId(),
		keyFile: useId(),
		jumpHost: useId(),
		startFolder: useId(),
		refresh: useId(),
		domain: useId(),
		share: useId(),
		davAuth: useId(),
		nextcloud: useId(),
		s3Preset: useId(),
		s3Value: useId(),
		s3Region: useId(),
		s3PathStyle: useId(),
		sessionToken: useId(),
		thumbnails: useId(),
		thumbnailMax: useId(),
		password: useId(),
		remember: useId(),
		hosts: useId(),
		result: useId(),
	};

	useEffect(() => {
		let live = true;
		client.support().then(
			(next) => live && setSupport(next),
			(error: unknown) => console.warn('could not read what can be connected to', error),
		);
		client.suggested().then(
			(next) => live && setSuggestions(next),
			() => {},
		);
		return () => {
			live = false;
			if (parseTimer.current) clearTimeout(parseTimer.current);
		};
	}, [client]);

	// An address the dialog opened with is read at once.
	useEffect(() => {
		if (request.mode === 'new' && request.address) readAddress(request.address);
		// Only on open.
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, []);

	// The protocols that are on follow Settings → Experimental while the dialog is open; the form's
	// own scheme stays listed (marked) when it is off, so editing a saved connection of it still shows it.
	const protocols = useConnectionsView((view) => view.protocols);
	const enabled = protocols?.schemes ?? support?.schemes ?? [form.scheme];
	const schemes = enabled.includes(form.scheme) ? enabled : [...enabled, form.scheme];
	const keyring = support ? support.keyring : 'noKeyring';

	const change = <K extends keyof ConnectForm>(key: K, value: ConnectForm[K]) => {
		setForm((now) =>
			key === 'scheme' ? withScheme(now, value as string) : { ...now, [key]: value },
		);
		setProblems((now) => ({ ...now, [key]: undefined }));
		setResult(null);
	};
	const family = familyOf(form.scheme);
	const methods = methodsFor(form.scheme);

	/** Reads `text` through Rust; resolves to whether it filled the form (or was empty), `false` when it was refused. */
	function readAddress(text: string): Promise<boolean> {
		const turn = ++parseTurn.current;
		if (text.trim() === '') {
			setAddressNote('');
			setAddressOff(false);
			setProblems((now) => ({ ...now, address: undefined }));
			return Promise.resolve(true);
		}
		const read = client.parseAddress(text).then(
			(parsed) => {
				if (turn !== parseTurn.current) return true;
				const next = formOf(parsed.draft, formRef.current);
				formRef.current = next;
				setForm(next);
				setProblems({});
				setAddressOff(false);
				setAddressNote(parsed.passwordDropped ? t('connect.address.passwordDropped') : '');
				return true;
			},
			(error: unknown) => {
				if (turn !== parseTurn.current) return false;
				setAddressNote('');
				setAddressOff(isVfsError(error) && error.kind === 'protocolOff');
				setProblems((now) => ({
					...now,
					address:
						isVfsError(error) && error.kind === 'unsupported'
							? tf('connect.address.unsupported', { scheme: error.what })
							: isVfsError(error) && error.kind === 'protocolOff'
								? tf('connect.address.protocolOff', { protocol: schemeLabel(error.scheme) })
								: t('connect.address.invalid'),
				}));
				return false;
			},
		);
		parsing.current = read;
		return read;
	}

	/**
	 * Makes the form say what the address says before it is used: a read still waiting for its
	 * delay starts now, and one under way is waited for. `null` when the address was refused.
	 */
	const settleAddress = async (): Promise<ConnectForm | null> => {
		if (parseTimer.current) {
			clearTimeout(parseTimer.current);
			parseTimer.current = null;
			if (!(await readAddress(addressRef.current))) return null;
		} else if (parsing.current && !(await parsing.current)) {
			return null;
		}
		return formRef.current;
	};

	const onAddress = (text: string) => {
		setAddress(text);
		addressRef.current = text;
		setResult(null);
		if (parseTimer.current) clearTimeout(parseTimer.current);
		parseTimer.current = setTimeout(() => {
			parseTimer.current = null;
			void readAddress(text);
		}, PARSE_DELAY_MS);
	};

	const load = (entry: ConnectionEntry | null) => {
		setEditing(entry?.connection.id ?? null);
		setForm(entry ? formOf(entry.connection) : emptyForm(schemes[0]));
		setAddress(entry?.location.display ?? '');
		setAddressNote('');
		setProblems({});
		setPassword('');
		setSessionToken('');
		setRemember(false);
		setResult(null);
	};

	/** The password typed with "Password" chosen, sent with the first attempt. */
	const firstAnswer = (current: ConnectForm): Answered | null => {
		if (password === '') return null;
		if (familyOf(current.scheme) === 's3') {
			const keyId = current.user.trim();
			// An access key is an id and its secret; the id is the connection's own field.
			return keyId === ''
				? null
				: {
						answer: {
							kind: 'accessKey',
							keyId,
							secret: password,
							sessionToken: sessionToken === '' ? null : sessionToken,
						},
						remember: keyring === null && remember,
					};
		}
		if (current.auth === 'token') {
			return {
				answer: { kind: 'passphrase', passphrase: password },
				remember: keyring === null && remember,
			};
		}
		return current.auth === 'password'
			? {
					answer: {
						kind: 'password',
						// The user as the login names it (`domain;user` on SMB).
						user: draftOf(current).user,
						password,
					},
					remember: keyring === null && remember,
				}
			: null;
	};

	function rememberFocus() {
		const active = document.activeElement;
		focusBack.current =
			active instanceof HTMLElement && active.closest('dialog') !== null ? active : null;
	}

	// An attempt over: the dialog is still open, so focus goes back into it if it was lost to the page.
	useEffect(() => {
		if (busy !== null) return;
		const back = focusBack.current;
		focusBack.current = null;
		if (!back?.isConnected || back.closest('dialog') === null) return;
		const active = document.activeElement;
		if (active === null || active === document.body || active === back.closest('dialog')) {
			back.focus();
		}
	}, [busy]);

	/** What is wrong before anything is sent: put under its field, as a refusal would be. */
	const checkForm = (current: ConnectForm): boolean => {
		const problem = formProblem(current);
		if (!problem) return true;
		setProblems({ [problem.field]: problem.message });
		setResult({ tone: 'failed', text: problem.message });
		return false;
	};

	/** The Nextcloud preset: Rust writes the address of the person's files, and the dialog reads it like a typed one. */
	const fillNextcloud = async () => {
		const server = `${form.host.trim()}${form.port.trim() ? `:${form.port.trim()}` : ''}`;
		try {
			const written = await client.nextcloudAddress(server, form.user.trim());
			setAddress(written);
			readAddress(written);
			setForm((now) => ({ ...now, nextcloud: true }));
		} catch (error) {
			refuse(error);
		}
	};

	/** Puts a refusal under its field, or says it as the result. */
	const refuse = (error: unknown) => {
		const problem = draftProblem(error);
		if (problem) {
			setProblems({ [problem.field]: problem.message });
			setResult({ tone: 'failed', text: problem.message });
			return;
		}
		setResult({ tone: 'failed', text: connectionErrorText(error) });
	};

	const attempt = async (then: 'test' | 'connect') => {
		if (busy) return;
		rememberFocus();
		setBusy(then);
		setResult(null);
		const current = await settleAddress();
		if (!current || !checkForm(current)) {
			setBusy(null);
			return;
		}
		const draft = draftOf(current);
		const outcome = await connectAnswering(
			(answer, keep) => client.test(draft, answer, keep),
			ask,
			firstAnswer(current),
		);
		setBusy(null);
		setPassword('');
		setSessionToken('');
		switch (outcome.kind) {
			case 'connected': {
				const tested = outcome.value;
				const note = rememberedText(tested.remembered);
				const text = [tf('connect.result.connected', { server: tested.location.display }), note]
					.filter(Boolean)
					.join(' ');
				announce(text);
				if (then === 'connect') {
					onOpen(tested.location);
					onClose();
					return;
				}
				setResult({ tone: 'ok', text });
				return;
			}
			case 'cancelled':
				setResult({ tone: 'failed', text: t('connect.result.cancelled') });
				return;
			case 'failed':
				refuse(outcome.error);
		}
	};

	const save = async () => {
		if (busy) return;
		rememberFocus();
		setBusy('save');
		setResult(null);
		try {
			const current = await settleAddress();
			if (!current || !checkForm(current)) return;
			const draft = draftOf(current);
			const entry = editing ? await client.update(editing, draft) : await client.add(draft);
			setEditing(entry.connection.id);
			setForm(formOf(entry.connection));
			setAddress(entry.location.display);
			const text = tf(editing ? 'connect.result.updated' : 'connect.result.saved', {
				name: entry.label,
			});
			setResult({ tone: 'ok', text });
			announce(text);
		} catch (error) {
			refuse(error);
		} finally {
			setBusy(null);
		}
	};

	const duplicate = async (entry: ConnectionEntry) => {
		try {
			const copy = await client.duplicate(
				entry.connection.id,
				tf('connect.copyName', { name: entry.label }),
			);
			load(copy);
			announce(tf('connect.result.duplicated', { name: copy.label }));
		} catch (error) {
			refuse(error);
		}
	};

	const forget = async () => {
		const entry = forgetting;
		setForgetting(null);
		if (!entry) return;
		try {
			const why = await client.remove(entry.connection.id, forgetLogin);
			if (editing === entry.connection.id) load(null);
			const text =
				why === null
					? tf('connect.result.removed', { name: entry.label })
					: tf('connect.result.removedKeyring', { name: entry.label, reason: keyringText(why) });
			setResult({ tone: 'ok', text });
			announce(text);
		} catch (error) {
			refuse(error);
		}
	};

	const submit = (event: FormEvent) => {
		event.preventDefault();
		void attempt('connect');
	};

	const working = busy !== null;
	const field = (
		key: FormField,
		id: string,
		label: MessageId,
		input: ReactNode,
		hint?: MessageId,
	) => (
		<div className={styles.field}>
			<label className={styles.label} htmlFor={id}>
				{t(label)}
			</label>
			{input}
			{hint && (
				<p id={`${id}-hint`} className={styles.hint}>
					{t(hint)}
				</p>
			)}
			<p id={`${id}-problem`} className={styles.problem} role="alert">
				{problems[key] ?? ''}
			</p>
		</div>
	);
	const described = (key: FormField, id: string, hint = false) =>
		[hint ? `${id}-hint` : '', problems[key] ? `${id}-problem` : ''].filter(Boolean).join(' ') ||
		undefined;
	const text = (
		key: Exclude<FormField, 'address' | 'scheme'>,
		id: string,
		value: string,
		hint = false,
	) => (
		<input
			id={id}
			className={styles.input}
			value={value}
			disabled={working}
			autoComplete="off"
			spellCheck={false}
			aria-invalid={problems[key] ? true : undefined}
			aria-describedby={described(key, id, hint)}
			onChange={(event) => change(key, event.target.value)}
		/>
	);

	return (
		<>
			<Dialog
				open
				size="large"
				title={editing ? t('connect.titleEdit') : t('connect.title')}
				description={t('connect.description')}
				initialFocus={`#${CSS.escape(ids.address)}`}
				onClose={() => {
					if (!working) onClose();
				}}
				footer={
					<DialogActions>
						<DialogButton variant="secondary" disabled={working} onClick={onClose}>
							{t('connect.cancel')}
						</DialogButton>
						<DialogButton
							variant="secondary"
							disabled={working}
							onClick={() => void attempt('test')}
						>
							{busy === 'test' ? t('connect.testing') : t('connect.test')}
						</DialogButton>
						<DialogButton variant="secondary" disabled={working} onClick={() => void save()}>
							{editing ? t('connect.saveChanges') : t('connect.save')}
						</DialogButton>
						<DialogButton
							variant="primary"
							disabled={working}
							onClick={() => void attempt('connect')}
						>
							{busy === 'connect' ? t('connect.connecting') : t('connect.connect')}
						</DialogButton>
					</DialogActions>
				}
			>
				<div className={styles.layout}>
					<section className={styles.saved} aria-labelledby={`${ids.hosts}-saved`}>
						<h3 id={`${ids.hosts}-saved`} className={styles.heading}>
							{t('connect.saved.heading')}
						</h3>
						<button
							type="button"
							className={styles.small}
							disabled={working}
							onClick={() => load(null)}
						>
							{t('connect.saved.new')}
						</button>
						{saved.length === 0 ? (
							<p className={styles.empty}>{t('connect.saved.empty')}</p>
						) : (
							<ul className={styles.list}>
								{saved.map((entry) => (
									<li
										key={entry.connection.id}
										className={styles.entry}
										data-current={entry.connection.id === editing ? '' : undefined}
									>
										<button
											type="button"
											className={styles.pick}
											disabled={working}
											aria-current={entry.connection.id === editing ? 'true' : undefined}
											aria-label={tf('connect.saved.edit', { name: entry.label })}
											onClick={() => load(entry)}
										>
											<span className={styles.pickLabel}>{entry.label}</span>
											<span className={styles.pickDetail}>{entry.location.display}</span>
										</button>
										<button
											type="button"
											className={styles.small}
											disabled={working}
											aria-label={tf('connect.saved.duplicate', { name: entry.label })}
											onClick={() => void duplicate(entry)}
										>
											{t('connect.saved.duplicateShort')}
										</button>
										<button
											type="button"
											className={styles.small}
											disabled={working}
											aria-label={tf('connect.saved.remove', { name: entry.label })}
											onClick={() => {
												setForgetLogin(true);
												setForgetting(entry);
											}}
										>
											{t('connect.saved.removeShort')}
										</button>
									</li>
								))}
							</ul>
						)}
					</section>
					<form className={styles.form} onSubmit={submit} noValidate>
						<div className={styles.field}>
							<label className={styles.label} htmlFor={ids.address}>
								{t('connect.field.address')}
							</label>
							<input
								id={ids.address}
								className={styles.input}
								value={address}
								disabled={working}
								placeholder={t(PLACEHOLDERS[form.scheme] ?? 'connect.address.placeholder')}
								autoComplete="off"
								spellCheck={false}
								aria-invalid={problems.address ? true : undefined}
								aria-describedby={`${ids.address}-hint ${ids.address}-problem`}
								onChange={(event) => onAddress(event.target.value)}
							/>
							<p id={`${ids.address}-hint`} className={styles.hint} role="status">
								{addressNote || t('connect.address.hint')}
							</p>
							<p id={`${ids.address}-problem`} className={styles.problem} role="alert">
								{problems.address ?? ''}
							</p>
							{problems.address && addressOff && <ExperimentalLink />}
						</div>
						<div className={styles.row}>
							{field(
								'host',
								ids.host,
								family === 's3' ? 'connect.field.bucket' : 'connect.field.host',
								<>
									<input
										id={ids.host}
										className={styles.input}
										value={form.host}
										disabled={working}
										list={suggestions.length > 0 ? `${ids.hosts}-list` : undefined}
										autoComplete="off"
										spellCheck={false}
										aria-invalid={problems.host ? true : undefined}
										aria-describedby={described('host', ids.host)}
										onChange={(event) => change('host', event.target.value)}
									/>
									{suggestions.length > 0 && (
										<datalist id={`${ids.hosts}-list`}>
											{suggestions.map((server) => (
												<option key={server.alias} value={server.alias} />
											))}
										</datalist>
									)}
								</>,
							)}
							{family !== 's3' &&
								field('port', ids.port, 'connect.field.port', text('port', ids.port, form.port))}
						</div>
						<div className={styles.row}>
							{field(
								'user',
								ids.user,
								family === 's3' ? 'connect.field.keyId' : 'connect.field.user',
								text('user', ids.user, form.user),
							)}
							{family === 'smb' &&
								field(
									'domain',
									ids.domain,
									'connect.field.domain',
									text('domain', ids.domain, form.domain, true),
									'connect.field.domainHint',
								)}
							{field(
								'scheme',
								ids.scheme,
								'connect.field.protocol',
								<select
									id={ids.scheme}
									className={styles.input}
									value={form.scheme}
									disabled={working}
									onChange={(event) => change('scheme', event.target.value)}
								>
									{schemes.map((scheme) => (
										<option key={scheme} value={scheme}>
											{enabled.includes(scheme)
												? schemeLabel(scheme)
												: tf('connect.scheme.off', { protocol: schemeLabel(scheme) })}
										</option>
									))}
								</select>,
							)}
						</div>
						{field(
							'name',
							ids.name,
							'connect.field.name',
							text('name', ids.name, form.name, true),
							'connect.field.nameHint',
						)}
						{family === 'smb' &&
							field(
								'startFolder',
								ids.share,
								'connect.field.share',
								<input
									id={ids.share}
									className={styles.input}
									value={form.startFolder.replace(/^\/+/, '')}
									disabled={working}
									autoComplete="off"
									spellCheck={false}
									aria-invalid={problems.startFolder ? true : undefined}
									aria-describedby={described('startFolder', ids.share, true)}
									onChange={(event) => {
										const folder = event.target.value.replace(/^\/+/, '');
										change('startFolder', folder === '' ? '' : `/${folder}`);
									}}
								/>,
								'connect.field.shareHint',
							)}
						{family === 'dav' && form.scheme === 'dav' && (
							<p className={styles.hint} role="note">
								{t('connect.dav.unencrypted')}
							</p>
						)}
						{family === 's3' && (
							<>
								<div className={styles.row}>
									<div className={styles.field}>
										<label className={styles.label} htmlFor={ids.s3Preset}>
											{t('connect.field.s3Service')}
										</label>
										<select
											id={ids.s3Preset}
											className={styles.input}
											value={form.s3Preset}
											disabled={working}
											onChange={(event) => {
												change('s3Preset', event.target.value);
												change('s3Value', '');
												change('s3PathStyle', null);
											}}
										>
											{S3_PRESETS.map((preset) => (
												<option key={preset.id} value={preset.id}>
													{t(preset.label)}
												</option>
											))}
										</select>
									</div>
									{s3Preset(form.s3Preset).input !== 'none' &&
										field(
											's3Value',
											ids.s3Value,
											S3_INPUT_LABEL[s3Preset(form.s3Preset).input],
											text('s3Value', ids.s3Value, form.s3Value, true),
											S3_INPUT_HINT[s3Preset(form.s3Preset).input],
										)}
								</div>
								{s3Preset(form.s3Preset).input !== 'region' &&
									field(
										's3Region',
										ids.s3Region,
										'connect.field.s3Region',
										text('s3Region', ids.s3Region, form.s3Region, true),
										'connect.field.s3RegionHint',
									)}
								<div className={styles.field}>
									<label className={styles.check} htmlFor={ids.s3PathStyle}>
										<input
											id={ids.s3PathStyle}
											type="checkbox"
											checked={form.s3PathStyle ?? s3Preset(form.s3Preset).pathStyle}
											disabled={working}
											aria-describedby={`${ids.s3PathStyle}-hint`}
											onChange={(event) => change('s3PathStyle', event.target.checked)}
										/>
										{t('connect.field.s3PathStyle')}
									</label>
									<p id={`${ids.s3PathStyle}-hint`} className={styles.hint}>
										{t('connect.field.s3PathStyleHint')}
									</p>
								</div>
								<p className={styles.hint} role="note">
									{t('connect.s3.moveNote')}
								</p>
							</>
						)}
						{family !== 's3' && (
							<fieldset className={styles.methods} disabled={working}>
								<legend>{t('connect.field.auth')}</legend>
								{methods.map((method) => (
									<label
										key={method}
										className={styles.check}
										title={t(METHODS[method].hint(family))}
									>
										<input
											type="radio"
											name={`${ids.password}-auth`}
											value={method}
											checked={form.auth === method}
											onChange={() => change('auth', method)}
										/>
										{t(METHODS[method].label)}
									</label>
								))}
							</fieldset>
						)}
						{family !== 's3' && <p className={styles.hint}>{t(METHODS[form.auth].hint(family))}</p>}
						{family === 'dav' && form.auth === 'password' && (
							<div className={styles.field}>
								<label className={styles.label} htmlFor={ids.davAuth}>
									{t('connect.field.davAuth')}
								</label>
								<select
									id={ids.davAuth}
									className={styles.input}
									value={form.davAuth}
									disabled={working}
									onChange={(event) =>
										change('davAuth', event.target.value as ConnectForm['davAuth'])
									}
								>
									<option value="auto">{t('connect.davAuth.auto')}</option>
									<option value="basic">{t('connect.davAuth.basic')}</option>
									<option value="digest">{t('connect.davAuth.digest')}</option>
								</select>
							</div>
						)}
						{family === 'dav' && (
							<div className={styles.field}>
								<label className={styles.check} htmlFor={ids.nextcloud}>
									<input
										id={ids.nextcloud}
										type="checkbox"
										checked={form.nextcloud}
										disabled={working}
										aria-describedby={`${ids.nextcloud}-hint`}
										onChange={(event) => change('nextcloud', event.target.checked)}
									/>
									{t('connect.field.nextcloud')}
								</label>
								<p id={`${ids.nextcloud}-hint`} className={styles.hint}>
									{t('connect.field.nextcloudHint')}
								</p>
								{form.nextcloud && (
									<button
										type="button"
										className={styles.small}
										disabled={working || form.host.trim() === '' || form.user.trim() === ''}
										onClick={() => void fillNextcloud()}
									>
										{t('connect.nextcloud.fill')}
									</button>
								)}
							</div>
						)}
						{form.auth === 'keyFile' &&
							field(
								'keyFile',
								ids.keyFile,
								'connect.field.keyFile',
								text('keyFile', ids.keyFile, form.keyFile, true),
								'connect.field.keyFileHint',
							)}
						{(form.auth === 'password' || form.auth === 'token' || family === 's3') && (
							<div className={styles.field}>
								<label className={styles.label} htmlFor={ids.password}>
									{family === 's3'
										? t('connect.field.secretKey')
										: form.auth === 'token'
											? t('connect.field.token')
											: t('connect.field.password')}
								</label>
								<input
									id={ids.password}
									type="password"
									className={styles.input}
									value={password}
									disabled={working}
									autoComplete="off"
									spellCheck={false}
									aria-describedby={`${ids.password}-hint`}
									onChange={(event) => setPassword(event.target.value)}
								/>
								<p id={`${ids.password}-hint`} className={styles.hint}>
									{family === 's3'
										? t('connect.field.secretKeyHint')
										: t('connect.field.passwordHint')}
								</p>
								{family === 's3' && (
									<div className={styles.field}>
										<label className={styles.label} htmlFor={ids.sessionToken}>
											{t('connect.field.sessionToken')}
										</label>
										<input
											id={ids.sessionToken}
											type="password"
											className={styles.input}
											value={sessionToken}
											disabled={working}
											autoComplete="off"
											spellCheck={false}
											aria-describedby={`${ids.sessionToken}-hint`}
											onChange={(event) => setSessionToken(event.target.value)}
										/>
										<p id={`${ids.sessionToken}-hint`} className={styles.hint}>
											{t('connect.field.sessionTokenHint')}
										</p>
									</div>
								)}
								{keyring === null ? (
									<label className={styles.check} htmlFor={ids.remember}>
										<input
											id={ids.remember}
											type="checkbox"
											checked={remember}
											disabled={working}
											onChange={(event) => setRemember(event.target.checked)}
										/>
										{t('connect.remember.label')}
									</label>
								) : (
									<p className={styles.hint} role="note">
										{tf('connect.remember.unavailable', { reason: keyringText(keyring) })}
									</p>
								)}
							</div>
						)}
						<details className={styles.more}>
							<summary>{t('connect.more')}</summary>
							{family === 'ssh' &&
								field(
									'jumpHost',
									ids.jumpHost,
									'connect.field.jumpHost',
									text('jumpHost', ids.jumpHost, form.jumpHost, true),
									'connect.field.jumpHostHint',
								)}
							{family !== 'smb' &&
								field(
									'startFolder',
									ids.startFolder,
									'connect.field.startFolder',
									text('startFolder', ids.startFolder, form.startFolder, true),
									'connect.field.startFolderHint',
								)}
							{field(
								'refreshSeconds',
								ids.refresh,
								'connect.field.refresh',
								text('refreshSeconds', ids.refresh, form.refreshSeconds, true),
								'connect.field.refreshHint',
							)}
							<div className={styles.field}>
								<label className={styles.label} htmlFor={ids.thumbnails}>
									{t('connect.field.thumbnails')}
								</label>
								<select
									id={ids.thumbnails}
									className={styles.input}
									value={form.thumbnails}
									disabled={working}
									aria-describedby={`${ids.thumbnails}-hint`}
									onChange={(event) =>
										change('thumbnails', event.target.value as ConnectForm['thumbnails'])
									}
								>
									{THUMBNAIL_CHOICES.map((choice) => (
										<option key={choice} value={choice}>
											{t(`connect.thumbnails.${choice}`)}
										</option>
									))}
								</select>
								<p id={`${ids.thumbnails}-hint`} className={styles.hint}>
									{t('connect.field.thumbnailsHint')}
								</p>
							</div>
							{form.thumbnails !== 'off' &&
								field(
									'thumbnailMaxMb',
									ids.thumbnailMax,
									'connect.field.thumbnailMaxMb',
									text('thumbnailMaxMb', ids.thumbnailMax, form.thumbnailMaxMb, true),
									'connect.field.thumbnailMaxMbHint',
								)}
						</details>
						<p id={ids.result} className={styles.result} role="status" data-tone={result?.tone}>
							{busy === 'test' || busy === 'connect'
								? t('connect.result.trying')
								: (result?.text ?? '')}
						</p>
						{/* Enter in a field connects. */}
						<button type="submit" hidden tabIndex={-1} aria-hidden="true" />
					</form>
				</div>
			</Dialog>
			<Dialog
				open={forgetting !== null}
				size="small"
				title={tf('connect.forget.title', { name: forgetting?.label ?? '' })}
				description={t('connect.forget.message')}
				initialFocus="[data-cancel]"
				onClose={() => setForgetting(null)}
				footer={
					<DialogActions>
						<DialogButton variant="secondary" data-cancel="" onClick={() => setForgetting(null)}>
							{t('connect.cancel')}
						</DialogButton>
						<DialogButton variant="danger" onClick={() => void forget()}>
							{t('connect.forget.confirm')}
						</DialogButton>
					</DialogActions>
				}
			>
				<label className={styles.check}>
					<input
						type="checkbox"
						checked={forgetLogin}
						onChange={(event) => setForgetLogin(event.target.checked)}
					/>
					{t('connect.forget.login')}
				</label>
			</Dialog>
		</>
	);
}
