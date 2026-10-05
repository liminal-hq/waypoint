// The questions a connection asks: sign in, trust an unknown host key, the changed-host-key warning and an untrusted certificate
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Dialog } from '@liminal-hq/waypoint-chrome/Dialog/Dialog';
import { DialogActions, DialogButton } from '@liminal-hq/waypoint-chrome/Dialog/DialogActions';
import type { AnswerInput } from '@liminal-hq/waypoint-protocol/generated/AnswerInput';
import type { AuthPrompt } from '@liminal-hq/waypoint-protocol/generated/AuthPrompt';
import type { KeyringUnavailable } from '@liminal-hq/waypoint-protocol/generated/KeyringUnavailable';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import { useId, useState, type FormEvent } from 'react';
import { isArchiveLocation } from '../archives/archiveNames';
import { t, tf } from '../i18n/messages';
import type { Answered } from './connectFlow';
import { keyringText } from './connectModel';
import styles from './Connect.module.css';

interface QuestionProps {
	/** The error that asks the question. */
	error: VfsError;
	/** Why a credential cannot be remembered here, or `null` when it can. */
	keyring: KeyringUnavailable | null;
	onAnswer(answered: Answered | null): void;
}

/** The dialog for the question `error` asks, or nothing for an error that asks none. */
export function QuestionDialog(props: QuestionProps) {
	switch (props.error.kind) {
		case 'authRequired':
			return <SignInDialog {...props} prompt={props.error.prompt} refused={false} />;
		case 'authFailed':
			// A refused S3 login asks for the access key again, not a password.
			return (
				<SignInDialog
					{...props}
					prompt={
						props.error.location.uri.startsWith('s3://')
							? { kind: 'accessKey', keyId: null }
							: { kind: 'password', user: null }
					}
					refused={true}
				/>
			);
		case 'hostKeyUnknown':
			return <HostKeyDialog {...props} />;
		case 'hostKeyChanged':
			return <HostKeyChangedDialog {...props} />;
		case 'certificateUntrusted':
			return <CertificateDialog {...props} />;
		default:
			return null;
	}
}

/** Where the question is about, for its title. */
function serverOf(error: VfsError): string {
	return 'location' in error && error.location ? error.location.display : '';
}

/** "Remember" when the keyring answers, or the line saying why it cannot (D147). */
function RememberChoice({
	keyring,
	checked,
	onChange,
	disabled,
}: {
	keyring: KeyringUnavailable | null;
	checked: boolean;
	onChange(on: boolean): void;
	disabled: boolean;
}) {
	const id = useId();
	if (keyring !== null) {
		return (
			<p className={styles.hint} role="note">
				{tf('connect.remember.unavailable', { reason: keyringText(keyring) })}
			</p>
		);
	}
	return (
		<div className={styles.field}>
			<label className={styles.check} htmlFor={id}>
				<input
					id={id}
					type="checkbox"
					checked={checked}
					disabled={disabled}
					aria-describedby={`${id}-hint`}
					onChange={(event) => onChange(event.target.checked)}
				/>
				{t('connect.remember.label')}
			</label>
			<p id={`${id}-hint`} className={styles.hint}>
				{t('connect.remember.hint')}
			</p>
		</div>
	);
}

/**
 * Asks for what a login needs: a password (and the user when the server does not know it), a key's
 * passphrase, the questions of a keyboard-interactive round, or an access key. Focus starts on the
 * first field; Enter signs in. A refused login says so above the fields. The text goes to Rust once
 * and is dropped with the dialog.
 */
function SignInDialog({
	error,
	keyring,
	onAnswer,
	prompt,
	refused,
}: QuestionProps & { prompt: AuthPrompt; refused: boolean }) {
	const [user, setUser] = useState(prompt.kind === 'password' ? (prompt.user ?? '') : '');
	const [secret, setSecret] = useState('');
	const [keyId, setKeyId] = useState(prompt.kind === 'accessKey' ? (prompt.keyId ?? '') : '');
	// An S3 session token, for temporary credentials: asked for the session only, never remembered.
	const [sessionToken, setSessionToken] = useState('');
	const tokenId = useId();
	const [answers, setAnswers] = useState<string[]>(
		prompt.kind === 'challenge' ? prompt.prompts.map(() => '') : [],
	);
	const [remember, setRemember] = useState(false);
	const firstId = useId();
	const secretId = useId();
	const noteId = useId();
	// An archive's password is kept in memory until Waypoint quits, and never saved (D162).
	const archive = 'location' in error && !!error.location && isArchiveLocation(error.location);
	const canRemember = prompt.kind !== 'challenge' && !archive;

	const answer = (): AnswerInput => {
		switch (prompt.kind) {
			case 'password':
				return { kind: 'password', user: user.trim() || null, password: secret };
			case 'passphrase':
				return { kind: 'passphrase', passphrase: secret };
			case 'challenge':
				return { kind: 'challenge', answers };
			case 'accessKey':
				return {
					kind: 'accessKey',
					keyId: keyId.trim(),
					secret,
					sessionToken: sessionToken === '' ? null : sessionToken,
				};
		}
	};
	const ready =
		prompt.kind === 'challenge'
			? true
			: secret !== '' && (prompt.kind !== 'accessKey' || keyId.trim() !== '');

	const submit = (event?: FormEvent) => {
		event?.preventDefault();
		if (!ready) return;
		onAnswer({ answer: answer(), remember: canRemember && keyring === null && remember });
	};

	const server = serverOf(error);
	const title =
		prompt.kind === 'passphrase'
			? tf('connect.signIn.passphraseTitle', { subject: prompt.subject })
			: tf('connect.signIn.title', { server });
	const askUser = prompt.kind === 'password' && !prompt.user;
	return (
		<Dialog
			open
			size="small"
			title={title}
			description={
				prompt.kind === 'challenge' && prompt.instructions
					? prompt.instructions
					: archive
						? t('archive.locked.dialogDescription')
						: t('connect.signIn.description')
			}
			initialFocus={`#${CSS.escape(firstId)}`}
			onClose={() => onAnswer(null)}
			footer={
				<DialogActions>
					<DialogButton variant="secondary" onClick={() => onAnswer(null)}>
						{t('connect.cancel')}
					</DialogButton>
					<DialogButton variant="primary" disabled={!ready} onClick={() => submit()}>
						{t('connect.signIn.confirm')}
					</DialogButton>
				</DialogActions>
			}
		>
			<form className={styles.form} onSubmit={submit} aria-describedby={noteId}>
				<p id={noteId} className={styles.problem} role="alert">
					{refused ? t('connect.signIn.refused') : ''}
				</p>
				{askUser && (
					<div className={styles.field}>
						<label className={styles.label} htmlFor={firstId}>
							{t('connect.field.user')}
						</label>
						<input
							id={firstId}
							className={styles.input}
							value={user}
							autoComplete="off"
							spellCheck={false}
							onChange={(event) => setUser(event.target.value)}
						/>
					</div>
				)}
				{prompt.kind === 'accessKey' && (
					<div className={styles.field}>
						<label className={styles.label} htmlFor={firstId}>
							{t('connect.signIn.keyId')}
						</label>
						<input
							id={firstId}
							className={styles.input}
							value={keyId}
							autoComplete="off"
							spellCheck={false}
							onChange={(event) => setKeyId(event.target.value)}
						/>
					</div>
				)}
				{prompt.kind === 'challenge' ? (
					prompt.prompts.map((question, index) => (
						<div className={styles.field} key={index}>
							<label
								className={styles.label}
								htmlFor={index === 0 ? firstId : `${firstId}-${index}`}
							>
								{question.text}
							</label>
							<input
								id={index === 0 ? firstId : `${firstId}-${index}`}
								className={styles.input}
								type={question.echo ? 'text' : 'password'}
								value={answers[index] ?? ''}
								autoComplete="off"
								spellCheck={false}
								onChange={(event) =>
									setAnswers((all) =>
										all.map((value, at) => (at === index ? event.target.value : value)),
									)
								}
							/>
						</div>
					))
				) : (
					<div className={styles.field}>
						<label
							className={styles.label}
							htmlFor={askUser || prompt.kind === 'accessKey' ? secretId : firstId}
						>
							{prompt.kind === 'passphrase'
								? t('connect.signIn.passphrase')
								: prompt.kind === 'accessKey'
									? t('connect.signIn.secretKey')
									: t('connect.signIn.password')}
						</label>
						<input
							id={askUser || prompt.kind === 'accessKey' ? secretId : firstId}
							className={styles.input}
							type="password"
							value={secret}
							autoComplete="off"
							spellCheck={false}
							onChange={(event) => setSecret(event.target.value)}
						/>
					</div>
				)}
				{prompt.kind === 'accessKey' && (
					<div className={styles.field}>
						<label className={styles.label} htmlFor={tokenId}>
							{t('connect.field.sessionToken')}
						</label>
						<input
							id={tokenId}
							className={styles.input}
							type="password"
							value={sessionToken}
							autoComplete="off"
							spellCheck={false}
							onChange={(event) => setSessionToken(event.target.value)}
						/>
					</div>
				)}
				{canRemember && (
					<RememberChoice
						keyring={keyring}
						checked={remember}
						onChange={setRemember}
						disabled={false}
					/>
				)}
				{/* Enter in any field signs in. */}
				<button type="submit" hidden tabIndex={-1} aria-hidden="true" />
			</form>
		</Dialog>
	);
}

/**
 * An SSH server whose key is not in `known_hosts` (D148): the host, the key type and its SHA-256
 * fingerprint, to compare with what the server's administrator gives. Focus starts on Cancel.
 */
function HostKeyDialog({ error, onAnswer }: QuestionProps) {
	if (error.kind !== 'hostKeyUnknown') return null;
	const { key } = error;
	const trust = (remember: boolean) =>
		onAnswer({
			answer: { kind: 'trustHostKey', fingerprint: key.fingerprint, remember },
			remember: false,
		});
	return (
		<Dialog
			open
			size="medium"
			title={tf('connect.hostKey.title', { host: key.host })}
			description={t('connect.hostKey.description')}
			initialFocus="[data-cancel]"
			onClose={() => onAnswer(null)}
			footer={
				<DialogActions>
					<DialogButton variant="secondary" data-cancel="" onClick={() => onAnswer(null)}>
						{t('connect.cancel')}
					</DialogButton>
					<DialogButton variant="secondary" onClick={() => trust(false)}>
						{t('connect.hostKey.once')}
					</DialogButton>
					<DialogButton variant="primary" onClick={() => trust(true)}>
						{t('connect.hostKey.remember')}
					</DialogButton>
				</DialogActions>
			}
		>
			<dl className={styles.facts}>
				<dt>{t('connect.hostKey.host')}</dt>
				<dd>{key.host}</dd>
				<dt>{t('connect.hostKey.algorithm')}</dt>
				<dd>{key.algorithm}</dd>
				<dt>{t('connect.hostKey.fingerprint')}</dt>
				<dd className={styles.fingerprint}>{key.fingerprint}</dd>
			</dl>
		</Dialog>
	);
}

/**
 * A server whose key differs from the one recorded (D148): a strong warning, the host, and the
 * recorded and offered keys side by side. It starts on Cancel and never connects silently; only the
 * explicit "Trust the new key" sends `trustChangedHostKey`, naming both fingerprints shown.
 */
function HostKeyChangedDialog({ error, onAnswer }: QuestionProps) {
	if (error.kind !== 'hostKeyChanged') return null;
	const { change } = error;
	return (
		<Dialog
			open
			size="medium"
			title={tf('connect.hostKeyChanged.title', { host: change.host })}
			description={t('connect.hostKeyChanged.description')}
			initialFocus="[data-cancel]"
			onClose={() => onAnswer(null)}
			footer={
				<DialogActions>
					<DialogButton variant="secondary" data-cancel="" onClick={() => onAnswer(null)}>
						{t('connect.cancel')}
					</DialogButton>
					<DialogButton
						variant="danger"
						onClick={() =>
							onAnswer({
								answer: {
									kind: 'trustChangedHostKey',
									recordedFingerprint: change.recordedFingerprint,
									offeredFingerprint: change.offeredFingerprint,
								},
								remember: false,
							})
						}
					>
						{t('connect.hostKeyChanged.trust')}
					</DialogButton>
				</DialogActions>
			}
		>
			<p className={styles.warning} role="alert">
				<strong className={styles.warningTitle}>{t('connect.hostKeyChanged.warningTitle')}</strong>
				{t('connect.hostKeyChanged.warning')}
			</p>
			<dl className={styles.facts}>
				<dt>{t('connect.hostKey.host')}</dt>
				<dd>{change.host}</dd>
				<dt>{t('connect.hostKeyChanged.recorded')}</dt>
				<dd>
					{change.recordedAlgorithm}{' '}
					<span className={styles.fingerprint}>{change.recordedFingerprint}</span>
				</dd>
				<dt>{t('connect.hostKeyChanged.offered')}</dt>
				<dd>
					{change.offeredAlgorithm}{' '}
					<span className={styles.fingerprint}>{change.offeredFingerprint}</span>
				</dd>
			</dl>
		</Dialog>
	);
}

/** A TLS certificate the system does not trust: who it is for, who issued it, why, and its fingerprint. Focus starts on Cancel. */
function CertificateDialog({ error, onAnswer }: QuestionProps) {
	if (error.kind !== 'certificateUntrusted') return null;
	const { certificate } = error;
	const trust = (remember: boolean) =>
		onAnswer({
			answer: { kind: 'trustCertificate', fingerprint: certificate.fingerprint, remember },
			remember: false,
		});
	return (
		<Dialog
			open
			size="medium"
			title={tf('connect.certificate.title', { server: serverOf(error) })}
			description={t('connect.certificate.description')}
			initialFocus="[data-cancel]"
			onClose={() => onAnswer(null)}
			footer={
				<DialogActions>
					<DialogButton variant="secondary" data-cancel="" onClick={() => onAnswer(null)}>
						{t('connect.cancel')}
					</DialogButton>
					<DialogButton variant="secondary" onClick={() => trust(false)}>
						{t('connect.certificate.once')}
					</DialogButton>
					<DialogButton variant="primary" onClick={() => trust(true)}>
						{t('connect.certificate.remember')}
					</DialogButton>
				</DialogActions>
			}
		>
			<dl className={styles.facts}>
				<dt>{t('connect.certificate.subject')}</dt>
				<dd>{certificate.subject}</dd>
				<dt>{t('connect.certificate.issuer')}</dt>
				<dd>{certificate.issuer}</dd>
				<dt>{t('connect.certificate.reason')}</dt>
				<dd>{certificate.reason}</dd>
				<dt>{t('connect.hostKey.fingerprint')}</dt>
				<dd className={styles.fingerprint}>{certificate.fingerprint}</dd>
			</dl>
		</Dialog>
	);
}
