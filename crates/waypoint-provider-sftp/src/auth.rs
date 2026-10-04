// Logging in: the agent, key files (asking for a passphrase), keyboard-interactive and password,
// in OpenSSH's order, with every secret from the person's answer or the app's credential source.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};
use std::sync::Arc;

use russh::client::{Handle, KeyboardInteractiveAuthResponse, Prompt};
use russh::keys::agent::AgentIdentity;
use russh::keys::{load_secret_key, PrivateKey, PrivateKeyWithHashAlg};
use russh::{MethodKind, MethodSet};
use waypoint_path::ConnectionKey;
use waypoint_protocol::{AuthPrompt, ChallengePrompt, Location, VfsError};
use waypoint_vfs::{Credential, CredentialSource, Secret};

use crate::client::Client;
use crate::errors::from_russh;
use crate::options::{home_dir, AgentSource};

/// Who logs in, and where the secrets come from.
pub(crate) struct Login<'a> {
    pub key: &'a ConnectionKey,
    pub user: &'a str,
    pub identity_files: &'a [PathBuf],
    pub agent: &'a AgentSource,
    pub credentials: &'a dyn CredentialSource,
    /// The person's answer to the question the last attempt asked.
    pub answer: Option<&'a Credential>,
    pub location: &'a Location,
}

/// Why a login stopped.
pub(crate) enum AuthStop {
    Error(VfsError),
    /// The server asked keyboard-interactive questions nobody has answered yet. The session is
    /// left waiting for them, so a second round (a one-time code after a password) can be
    /// answered on the same login; `prompt` is what to ask, `count` how many answers it takes.
    Waiting {
        prompt: AuthPrompt,
        count: usize,
    },
}

impl From<VfsError> for AuthStop {
    fn from(error: VfsError) -> Self {
        AuthStop::Error(error)
    }
}

/// What has been learned on the way.
#[derive(Default)]
struct Tally {
    /// The first thing the person could be asked for.
    needed: Option<AuthPrompt>,
    /// A secret was offered and refused.
    refused: bool,
}

impl Tally {
    fn need(&mut self, prompt: AuthPrompt) {
        self.needed.get_or_insert(prompt);
    }

    fn finish(self, location: &Location) -> AuthStop {
        let location = location.clone();
        AuthStop::Error(match (self.refused, self.needed) {
            (false, Some(prompt)) => VfsError::AuthRequired {
                location,
                prompt: Box::new(prompt),
            },
            _ => VfsError::AuthFailed { location },
        })
    }
}

enum Step {
    Done,
    Remaining(MethodSet),
}

fn has(methods: &MethodSet, kind: MethodKind) -> bool {
    methods.contains(&kind)
}

impl Login<'_> {
    fn russh(&self, error: russh::Error) -> AuthStop {
        AuthStop::Error(from_russh(&error, self.location))
    }

    fn password(&self) -> Option<Secret> {
        if let Some(Credential::Password { password, .. }) = self.answer {
            return Some(password.clone());
        }
        let prompt = AuthPrompt::Password {
            user: Some(self.user.to_owned()),
        };
        match self.credentials.credential(self.key, &prompt) {
            Some(Credential::Password { password, .. }) => Some(password),
            _ => None,
        }
    }

    fn rejected(&self, prompt: &AuthPrompt) {
        self.credentials.rejected(self.key, prompt);
    }

    /// Logs in from the start.
    pub(crate) async fn authenticate(&self, handle: &mut Handle<Client>) -> Result<(), AuthStop> {
        let mut methods = match handle
            .authenticate_none(self.user)
            .await
            .map_err(|e| self.russh(e))?
        {
            russh::client::AuthResult::Success => return Ok(()),
            russh::client::AuthResult::Failure {
                remaining_methods, ..
            } => remaining_methods,
        };
        let mut tally = Tally::default();
        if has(&methods, MethodKind::PublicKey) {
            match self.agent_login(handle).await? {
                Some(Step::Done) => return Ok(()),
                Some(Step::Remaining(rest)) => methods = rest,
                None => {}
            }
        }
        if has(&methods, MethodKind::PublicKey) {
            match self.key_files(handle, &mut tally).await? {
                Some(Step::Done) => return Ok(()),
                Some(Step::Remaining(rest)) => methods = rest,
                None => {}
            }
        }
        self.interactive_then_password(handle, methods, tally, false)
            .await
    }

    /// Answers the keyboard-interactive round a previous attempt left waiting.
    pub(crate) async fn resume(
        &self,
        handle: &mut Handle<Client>,
        count: usize,
    ) -> Result<(), AuthStop> {
        let answers = match self.answer {
            Some(Credential::Challenge(answers)) if answers.len() == count => answers
                .iter()
                .map(|answer| answer.expose_str().unwrap_or_default().to_owned())
                .collect(),
            Some(Credential::Password { password, .. }) if count == 1 => {
                vec![password.expose_str().unwrap_or_default().to_owned()]
            }
            _ => {
                return Err(AuthStop::Error(VfsError::AuthFailed {
                    location: self.location.clone(),
                }))
            }
        };
        let response = handle
            .authenticate_keyboard_interactive_respond(answers)
            .await
            .map_err(|e| self.russh(e))?;
        let mut tally = Tally::default();
        match self.rounds(handle, response, &mut tally, true).await? {
            Step::Done => Ok(()),
            Step::Remaining(methods) => {
                tally.refused = true;
                self.interactive_then_password(handle, methods, tally, true)
                    .await
            }
        }
    }

    async fn interactive_then_password(
        &self,
        handle: &mut Handle<Client>,
        mut methods: MethodSet,
        mut tally: Tally,
        skip_interactive: bool,
    ) -> Result<(), AuthStop> {
        if !skip_interactive && has(&methods, MethodKind::KeyboardInteractive) {
            let response = handle
                .authenticate_keyboard_interactive_start(self.user, None)
                .await
                .map_err(|e| self.russh(e))?;
            match self.rounds(handle, response, &mut tally, false).await? {
                Step::Done => return Ok(()),
                Step::Remaining(rest) => methods = rest,
            }
        }
        if has(&methods, MethodKind::Password) {
            let prompt = AuthPrompt::Password {
                user: Some(self.user.to_owned()),
            };
            match self.password() {
                Some(password) => {
                    let result = handle
                        .authenticate_password(self.user, password.expose_str().unwrap_or_default())
                        .await
                        .map_err(|e| self.russh(e))?;
                    if result.success() {
                        return Ok(());
                    }
                    tally.refused = true;
                    self.rejected(&prompt);
                }
                None => tally.need(prompt),
            }
        }
        Err(tally.finish(self.location))
    }

    /// Runs keyboard-interactive rounds until the server decides. `answered` says the first
    /// response already carried the person's answers.
    async fn rounds(
        &self,
        handle: &mut Handle<Client>,
        mut response: KeyboardInteractiveAuthResponse,
        tally: &mut Tally,
        mut answered: bool,
    ) -> Result<Step, AuthStop> {
        let mut offered_answer = !answered && matches!(self.answer, Some(Credential::Challenge(_)));
        loop {
            match response {
                KeyboardInteractiveAuthResponse::Success => return Ok(Step::Done),
                KeyboardInteractiveAuthResponse::Failure {
                    remaining_methods, ..
                } => {
                    if answered {
                        tally.refused = true;
                    }
                    return Ok(Step::Remaining(remaining_methods));
                }
                KeyboardInteractiveAuthResponse::InfoRequest {
                    name,
                    instructions,
                    prompts,
                } => {
                    let prompt = challenge_prompt(self.user, &name, &instructions, &prompts);
                    let answers = if prompts.is_empty() {
                        Some(Vec::new())
                    } else {
                        let from_answer = match (offered_answer, self.answer) {
                            (true, Some(Credential::Challenge(answers)))
                                if answers.len() == prompts.len() =>
                            {
                                Some(answers.clone())
                            }
                            _ => None,
                        };
                        offered_answer = false;
                        from_answer.or_else(|| self.challenge_answers(&prompt, &prompts))
                    };
                    let Some(answers) = answers else {
                        return Err(AuthStop::Waiting {
                            prompt,
                            count: prompts.len(),
                        });
                    };
                    answered |= !answers.is_empty();
                    let answers = answers
                        .iter()
                        .map(|answer| answer.expose_str().unwrap_or_default().to_owned())
                        .collect();
                    response = handle
                        .authenticate_keyboard_interactive_respond(answers)
                        .await
                        .map_err(|e| self.russh(e))?;
                }
            }
        }
    }

    /// Answers a round from what is at hand: a single hidden password question takes the
    /// password, anything else asks the credential source.
    fn challenge_answers(&self, prompt: &AuthPrompt, prompts: &[Prompt]) -> Option<Vec<Secret>> {
        if let AuthPrompt::Password { .. } = prompt {
            return self.password().map(|password| vec![password]);
        }
        match self.credentials.credential(self.key, prompt) {
            Some(Credential::Challenge(answers)) if answers.len() == prompts.len() => Some(answers),
            _ => None,
        }
    }

    async fn agent_login(&self, handle: &mut Handle<Client>) -> Result<Option<Step>, AuthStop> {
        let Some(mut agent) = connect_agent(self.agent).await else {
            return Ok(None);
        };
        let identities = match agent.request_identities().await {
            Ok(identities) => identities,
            Err(error) => {
                log::debug!("ssh agent: {error}");
                return Ok(None);
            }
        };
        let mut last = None;
        for identity in identities {
            let AgentIdentity::PublicKey { key, .. } = identity else {
                continue;
            };
            let hash = if key.algorithm().is_rsa() {
                handle
                    .best_supported_rsa_hash()
                    .await
                    .ok()
                    .flatten()
                    .flatten()
            } else {
                None
            };
            match handle
                .authenticate_publickey_with(self.user, key, hash, &mut agent)
                .await
            {
                Ok(russh::client::AuthResult::Success) => return Ok(Some(Step::Done)),
                Ok(russh::client::AuthResult::Failure {
                    remaining_methods, ..
                }) => last = Some(Step::Remaining(remaining_methods)),
                Err(error) => log::debug!("ssh agent signing: {error:?}"),
            }
        }
        Ok(last)
    }

    async fn key_files(
        &self,
        handle: &mut Handle<Client>,
        tally: &mut Tally,
    ) -> Result<Option<Step>, AuthStop> {
        let mut last = None;
        let mut passphrase_offered = false;
        let mut passphrase_worked = false;
        for path in self.identity_files {
            let key = match load_secret_key(path, None) {
                Ok(key) => key,
                Err(russh::keys::Error::KeyIsEncrypted) => {
                    let prompt = AuthPrompt::Passphrase {
                        subject: subject(path),
                    };
                    let passphrase = match self.answer {
                        Some(Credential::Passphrase(passphrase)) => Some(passphrase.clone()),
                        _ => match self.credentials.credential(self.key, &prompt) {
                            Some(Credential::Passphrase(passphrase)) => Some(passphrase),
                            _ => None,
                        },
                    };
                    let Some(passphrase) = passphrase else {
                        tally.need(prompt);
                        continue;
                    };
                    passphrase_offered = true;
                    match load_secret_key(path, passphrase.expose_str()) {
                        Ok(key) => {
                            passphrase_worked = true;
                            key
                        }
                        Err(_) => {
                            self.rejected(&prompt);
                            continue;
                        }
                    }
                }
                Err(error) => {
                    log::debug!("ssh key {}: {error}", path.display());
                    continue;
                }
            };
            match self.key_login(handle, key).await? {
                Step::Done => return Ok(Some(Step::Done)),
                remaining => last = Some(remaining),
            }
        }
        if passphrase_offered && !passphrase_worked {
            tally.refused = true;
        }
        Ok(last)
    }

    async fn key_login(
        &self,
        handle: &mut Handle<Client>,
        key: PrivateKey,
    ) -> Result<Step, AuthStop> {
        let hash = if key.algorithm().is_rsa() {
            handle
                .best_supported_rsa_hash()
                .await
                .ok()
                .flatten()
                .flatten()
        } else {
            None
        };
        let result = handle
            .authenticate_publickey(self.user, PrivateKeyWithHashAlg::new(Arc::new(key), hash))
            .await
            .map_err(|e| self.russh(e))?;
        Ok(match result {
            russh::client::AuthResult::Success => Step::Done,
            russh::client::AuthResult::Failure {
                remaining_methods, ..
            } => Step::Remaining(remaining_methods),
        })
    }
}

/// What a keyboard-interactive round asks. One hidden question about a password (PAM's usual
/// "Password:") is asked as a password, so a remembered password answers it.
fn challenge_prompt(user: &str, name: &str, instructions: &str, prompts: &[Prompt]) -> AuthPrompt {
    if let [only] = prompts {
        if !only.echo && only.prompt.to_lowercase().contains("password") {
            return AuthPrompt::Password {
                user: Some(user.to_owned()),
            };
        }
    }
    AuthPrompt::Challenge {
        name: name.to_owned(),
        instructions: instructions.to_owned(),
        prompts: prompts
            .iter()
            .map(|prompt| ChallengePrompt {
                text: prompt.prompt.clone(),
                echo: prompt.echo,
            })
            .collect(),
    }
}

/// A key file as people read it: `~/.ssh/id_ed25519`.
fn subject(path: &Path) -> String {
    if let Some(rest) = home_dir().and_then(|home| path.strip_prefix(home).ok().map(Path::to_owned))
    {
        return format!("~/{}", rest.display()).replace('\\', "/");
    }
    path.display().to_string()
}

type Agent = russh::keys::agent::client::AgentClient<
    Box<dyn russh::keys::agent::client::AgentStream + Send + Unpin + 'static>,
>;

async fn connect_agent(source: &AgentSource) -> Option<Agent> {
    use russh::keys::agent::client::AgentClient;
    let connected = match source {
        AgentSource::None => return None,
        #[cfg(unix)]
        AgentSource::System => AgentClient::connect_env().await.map(AgentClient::dynamic),
        #[cfg(unix)]
        AgentSource::Path(path) => AgentClient::connect_uds(path)
            .await
            .map(AgentClient::dynamic),
        #[cfg(windows)]
        AgentSource::System => {
            match AgentClient::connect_named_pipe(r"\\.\pipe\openssh-ssh-agent").await {
                Ok(agent) => Ok(agent.dynamic()),
                Err(_) => AgentClient::connect_pageant()
                    .await
                    .map(AgentClient::dynamic),
            }
        }
        #[cfg(windows)]
        AgentSource::Path(path) => AgentClient::connect_named_pipe(path)
            .await
            .map(AgentClient::dynamic),
    };
    match connected {
        Ok(agent) => Some(agent),
        Err(error) => {
            log::debug!("ssh agent unavailable: {error}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lone_hidden_password_question_is_asked_as_a_password() {
        let prompt = |text: &str, echo| Prompt {
            prompt: text.to_owned(),
            echo,
        };
        assert_eq!(
            challenge_prompt("me", "", "", &[prompt("Password: ", false)]),
            AuthPrompt::Password {
                user: Some("me".into())
            }
        );
        let AuthPrompt::Challenge { prompts, .. } = challenge_prompt(
            "me",
            "otp",
            "Enter your code",
            &[prompt("Verification code: ", true)],
        ) else {
            panic!("a code is a challenge");
        };
        assert_eq!(prompts[0].text, "Verification code: ");
        assert!(prompts[0].echo);
    }
}
