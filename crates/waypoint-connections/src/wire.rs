// What the page sends and is sent about connections beyond the store: answers, typed addresses, what is supported and suggested servers
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use waypoint_path::{ConnectionKey, VfsPath};
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{ConnectAnswer, Credential, Secret};

use crate::credentials::{KeyringUnavailable, SecretKind, SecretStore};
use crate::manager::ConnectionStatus;
use crate::model::{check_draft, host_text, ConnectionDraft};
use crate::store::ConnectionsSnapshot;

/// The person's answer to a connection's question, as the dialog sends it. It crosses IPC once,
/// from the dialog to Rust, and is turned into a `ConnectAnswer` at once: its text moves into a
/// `Secret`, which is zeroed when dropped. It has no `Debug` and no `Serialize`.
#[derive(Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum AnswerInput {
    Password {
        user: Option<String>,
        password: String,
    },
    Passphrase {
        passphrase: String,
    },
    /// The answers to a keyboard-interactive round, in the order of its prompts.
    Challenge {
        answers: Vec<String>,
    },
    AccessKey {
        key_id: String,
        secret: String,
    },
    /// Trust an unknown SSH host key, named by the fingerprint the dialog showed.
    TrustHostKey {
        fingerprint: String,
        remember: bool,
    },
    /// The explicit "Trust the new key" action on a changed host key, naming both fingerprints.
    TrustChangedHostKey {
        recorded_fingerprint: String,
        offered_fingerprint: String,
    },
    TrustCertificate {
        fingerprint: String,
        remember: bool,
    },
}

impl From<AnswerInput> for ConnectAnswer {
    fn from(input: AnswerInput) -> Self {
        match input {
            AnswerInput::Password { user, password } => {
                ConnectAnswer::Credential(Credential::Password {
                    user,
                    password: Secret::from(password),
                })
            }
            AnswerInput::Passphrase { passphrase } => {
                ConnectAnswer::Credential(Credential::Passphrase(Secret::from(passphrase)))
            }
            AnswerInput::Challenge { answers } => ConnectAnswer::Credential(Credential::Challenge(
                answers.into_iter().map(Secret::from).collect(),
            )),
            AnswerInput::AccessKey { key_id, secret } => {
                ConnectAnswer::Credential(Credential::AccessKey {
                    key_id,
                    secret: Secret::from(secret),
                })
            }
            AnswerInput::TrustHostKey {
                fingerprint,
                remember,
            } => ConnectAnswer::TrustHostKey {
                fingerprint,
                remember,
            },
            AnswerInput::TrustChangedHostKey {
                recorded_fingerprint,
                offered_fingerprint,
            } => ConnectAnswer::TrustChangedHostKey {
                recorded_fingerprint,
                offered_fingerprint,
            },
            AnswerInput::TrustCertificate {
                fingerprint,
                remember,
            } => ConnectAnswer::TrustCertificate {
                fingerprint,
                remember,
            },
        }
    }
}

/// What the Connect dialog can offer here: the protocols a provider serves, and whether a login
/// can be remembered (and why not).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct ConnectionSupport {
    /// The server schemes a provider is registered for (`sftp`, …), in order.
    pub schemes: Vec<String>,
    /// Why "Remember" cannot be offered; `None` when the keyring answers.
    pub keyring: Option<KeyringUnavailable>,
}

impl ConnectionSupport {
    pub fn new(schemes: Vec<String>, keyring: &dyn SecretStore) -> Self {
        Self {
            schemes,
            keyring: keyring.status().err(),
        }
    }
}

/// What a window reads when it opens: the saved connections, the recent servers, and the state of
/// every login the manager knows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct ConnectionsOverview {
    pub connections: ConnectionsSnapshot,
    pub statuses: Vec<ConnectionStatus>,
}

/// A host from `~/.ssh/config` the Connect dialog offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct SuggestedServer {
    /// The `Host` alias as written in the file.
    pub alias: String,
    /// The server's root, `sftp://alias/`.
    pub location: Location,
}

impl SuggestedServer {
    /// The suggestions of `aliases`, leaving out what is not a host name an address can hold.
    pub fn from_aliases(aliases: impl IntoIterator<Item = String>) -> Vec<Self> {
        aliases
            .into_iter()
            .filter_map(|alias| {
                let draft = ConnectionDraft {
                    scheme: "sftp".into(),
                    host: alias.clone(),
                    ..ConnectionDraft::default()
                };
                let checked = check_draft(&draft).ok()?;
                Some(Self {
                    alias,
                    location: VfsPath::Remote(checked.root).to_location(),
                })
            })
            .collect()
    }
}

/// A server address as typed, read into the dialog's fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct ParsedAddress {
    /// The fields the address fills in (scheme, host, port, user, start folder).
    pub draft: ConnectionDraft,
    /// The address in its canonical form.
    pub location: Location,
    /// The login it belongs to.
    pub key: String,
    /// A password was written in the address and dropped (D147).
    pub password_dropped: bool,
}

/// Reads a typed server address. A scheme no provider serves is `Unsupported`; text that is not a
/// server address is `InvalidLocation`.
pub fn parse_address(text: &str, serves: &dyn Fn(&str) -> bool) -> Result<ParsedAddress, VfsError> {
    let invalid = || VfsError::InvalidLocation {
        input: text.to_owned(),
    };
    let trimmed = text.trim();
    let (path, dropped) = VfsPath::parse_input_reporting(trimmed).map_err(|_| invalid())?;
    let VfsPath::Remote(remote) = &path else {
        return Err(invalid());
    };
    let scheme = remote.scheme().as_str();
    if !serves(scheme) {
        return Err(VfsError::Unsupported {
            what: scheme.to_owned(),
        });
    }
    let start = (!remote.is_root()).then(|| {
        let mut out = String::new();
        for segment in remote.segments() {
            out.push('/');
            out.push_str(&String::from_utf8_lossy(segment));
        }
        out
    });
    let draft = check_draft(&ConnectionDraft {
        scheme: scheme.to_owned(),
        host: host_text(&remote.authority().host),
        port: remote.authority().port,
        user: remote.authority().user.clone(),
        start_folder: start,
        ..ConnectionDraft::default()
    })
    .map_err(|_| invalid())?
    .draft;
    let key: ConnectionKey = remote.connection_key();
    Ok(ParsedAddress {
        draft,
        location: path.to_location(),
        key: key.as_str().to_owned(),
        password_dropped: dropped,
    })
}

/// A keyring that is never there, for an app with no `secrets` plugin: every login is asked for
/// and lasts for the session.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoKeyring;

impl SecretStore for NoKeyring {
    fn status(&self) -> Result<(), KeyringUnavailable> {
        Err(KeyringUnavailable::NoKeyring)
    }
    fn store(&self, _: &ConnectionKey, _: SecretKind, _: Secret) -> Result<(), KeyringUnavailable> {
        Err(KeyringUnavailable::NoKeyring)
    }
    fn fetch(
        &self,
        _: &ConnectionKey,
        _: SecretKind,
    ) -> Result<Option<Secret>, KeyringUnavailable> {
        Err(KeyringUnavailable::NoKeyring)
    }
    fn forget(&self, _: &ConnectionKey) -> Result<usize, KeyringUnavailable> {
        Err(KeyringUnavailable::NoKeyring)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sftp_only(scheme: &str) -> bool {
        scheme == "sftp"
    }

    #[test]
    fn a_typed_address_fills_the_fields_and_drops_a_password() {
        let parsed =
            parse_address(" SFTP://me:hunter2@NAS.lan:2222/srv/./media ", &sftp_only).unwrap();
        assert!(parsed.password_dropped);
        assert_eq!(parsed.location.uri, "sftp://me@nas.lan:2222/srv/media");
        assert_eq!(parsed.key, "sftp://me@nas.lan:2222");
        assert_eq!(parsed.draft.host, "nas.lan");
        assert_eq!(parsed.draft.port, Some(2222));
        assert_eq!(parsed.draft.user.as_deref(), Some("me"));
        assert_eq!(parsed.draft.start_folder.as_deref(), Some("/srv/media"));
        assert!(!format!("{parsed:?}").contains("hunter2"));
    }

    #[test]
    fn what_is_not_a_served_server_address_is_named() {
        assert!(matches!(
            parse_address("smb://files/share", &sftp_only),
            Err(VfsError::Unsupported { what }) if what == "smb"
        ));
        assert!(matches!(
            parse_address("/home/me", &sftp_only),
            Err(VfsError::InvalidLocation { .. })
        ));
        assert!(matches!(
            parse_address("sftp://", &sftp_only),
            Err(VfsError::InvalidLocation { .. })
        ));
    }

    #[test]
    fn an_answer_reads_from_the_dialogs_json_into_secrets() {
        let input: AnswerInput =
            serde_json::from_str(r#"{"kind":"password","user":"me","password":"hunter2"}"#)
                .unwrap();
        let answer = ConnectAnswer::from(input);
        assert!(!format!("{answer:?}").contains("hunter2"));
        let changed: AnswerInput = serde_json::from_str(
            r#"{"kind":"trustChangedHostKey","recordedFingerprint":"SHA256:a","offeredFingerprint":"SHA256:b"}"#,
        )
        .unwrap();
        assert_eq!(
            ConnectAnswer::from(changed),
            ConnectAnswer::TrustChangedHostKey {
                recorded_fingerprint: "SHA256:a".into(),
                offered_fingerprint: "SHA256:b".into()
            }
        );
    }

    #[test]
    fn suggestions_leave_out_patterns_that_are_not_hosts() {
        let found = SuggestedServer::from_aliases(["nas".to_owned(), "bad host".to_owned()]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].location.uri, "sftp://nas/");
    }
}
