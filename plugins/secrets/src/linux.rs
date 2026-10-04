// The Linux keyring: the Secret Service on the session bus and, in a Flatpak, the Secret portal, both through `oo7`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use oo7::ashpd;
use oo7::{dbus, file, Keyring};
use tokio::sync::Mutex;

use crate::backend::{Backend, BoxFuture};
use crate::error::{Result, SecretsError};
use crate::models::{Flavour, PluginStatus, Reason, Secret, SecretId};

const NO_KEYRING: &str =
    "no keyring is running: start GNOME Keyring, KWallet's Secret Service bridge or KeePassXC's Secret Service integration";
const LOCKED: &str = "the keyring is locked and was not unlocked";

/// The system keyring. Items carry the attributes `application` (the app's identifier), `service`, `account` and `kind`, so another program's items are never touched and the keyring's own manager can find ours.
pub struct Platform {
    namespace: String,
    keyring: Mutex<Option<Arc<Keyring>>>,
    /// Set when an unlock was refused or dismissed, so the status can say the keyring stays locked; cleared by the next success.
    unlock_refused: AtomicBool,
}

impl Platform {
    pub fn new(namespace: String) -> Self {
        Platform {
            namespace,
            keyring: Mutex::new(None),
            unlock_refused: AtomicBool::new(false),
        }
    }

    /// The connection, made on first use and again after one that broke.
    async fn keyring(&self) -> Result<Arc<Keyring>> {
        let mut guard = self.keyring.lock().await;
        if let Some(keyring) = guard.as_ref() {
            return Ok(keyring.clone());
        }
        let keyring = Arc::new(Keyring::new().await.map_err(classify)?);
        *guard = Some(keyring.clone());
        Ok(keyring)
    }

    /// Runs one action; a failure that means the connection is gone drops it, so the next action reconnects.
    async fn with_keyring<T, F, Fut>(&self, action: F) -> Result<T>
    where
        F: FnOnce(Arc<Keyring>) -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let keyring = self.keyring().await?;
        let outcome = action(keyring).await;
        if matches!(outcome, Err(SecretsError::NoKeyring)) {
            *self.keyring.lock().await = None;
        }
        outcome
    }

    /// Unlocks the collection when it is locked (the desktop shows its own prompt) and notes whether that worked.
    async fn ensure_unlocked(&self, keyring: &Keyring) -> Result<()> {
        if !keyring.is_locked().await.map_err(classify)? {
            self.unlock_refused.store(false, Ordering::Relaxed);
            return Ok(());
        }
        match keyring.unlock().await.map_err(classify) {
            Ok(()) if !keyring.is_locked().await.map_err(classify)? => {
                self.unlock_refused.store(false, Ordering::Relaxed);
                Ok(())
            }
            Ok(()) => {
                self.unlock_refused.store(true, Ordering::Relaxed);
                Err(SecretsError::Locked)
            }
            Err(error) => {
                if matches!(error, SecretsError::Locked | SecretsError::Dismissed) {
                    self.unlock_refused.store(true, Ordering::Relaxed);
                }
                Err(error)
            }
        }
    }

    fn attributes(
        &self,
        service: &str,
        account: &str,
        kind: Option<&str>,
    ) -> HashMap<String, String> {
        let mut attributes = HashMap::from([
            ("application".to_string(), self.namespace.clone()),
            ("service".to_string(), service.to_string()),
            ("account".to_string(), account.to_string()),
        ]);
        if let Some(kind) = kind {
            attributes.insert("kind".to_string(), kind.to_string());
        }
        attributes
    }

    fn attributes_of(&self, id: &SecretId) -> HashMap<String, String> {
        self.attributes(&id.service, &id.account, Some(id.kind.as_str()))
    }
}

/// Maps a `oo7` error to the plugin's typed one, never carrying more than the library's description.
fn classify(error: oo7::Error) -> SecretsError {
    match error {
        oo7::Error::DBus(error) => classify_dbus(error),
        oo7::Error::File(error) => classify_file(error),
    }
}

fn classify_dbus(error: dbus::Error) -> SecretsError {
    match error {
        dbus::Error::Dismissed => SecretsError::Dismissed,
        dbus::Error::Service(dbus::ServiceError::IsLocked(_)) => SecretsError::Locked,
        dbus::Error::Service(dbus::ServiceError::ZBus(error)) | dbus::Error::ZBus(error) => {
            classify_zbus(&error)
        }
        dbus::Error::NotFound(_) | dbus::Error::Service(dbus::ServiceError::NoSuchObject(_)) => {
            SecretsError::NoKeyring
        }
        dbus::Error::IO(_) => SecretsError::NoKeyring,
        other => SecretsError::failed(other.to_string()),
    }
}

fn classify_zbus(error: &oo7::zbus::Error) -> SecretsError {
    match error {
        oo7::zbus::Error::Address(_)
        | oo7::zbus::Error::InputOutput(_)
        | oo7::zbus::Error::Connection(..)
        | oo7::zbus::Error::Handshake(_) => SecretsError::NoKeyring,
        oo7::zbus::Error::FDO(error) => match error.as_ref() {
            oo7::zbus::fdo::Error::ServiceUnknown(_)
            | oo7::zbus::fdo::Error::NameHasNoOwner(_)
            | oo7::zbus::fdo::Error::NoReply(_)
            | oo7::zbus::fdo::Error::NoServer(_)
            | oo7::zbus::fdo::Error::BadAddress(_)
            | oo7::zbus::fdo::Error::Disconnected(_) => SecretsError::NoKeyring,
            oo7::zbus::fdo::Error::ZBus(inner) => classify_zbus(inner),
            _ => SecretsError::failed(error.to_string()),
        },
        oo7::zbus::Error::MethodError(name, _, _) => match name.as_str() {
            "org.freedesktop.DBus.Error.ServiceUnknown"
            | "org.freedesktop.DBus.Error.NameHasNoOwner"
            | "org.freedesktop.DBus.Error.NoReply"
            | "org.freedesktop.DBus.Error.Spawn.ServiceNotFound"
            | "org.freedesktop.DBus.Error.Spawn.ChildExited" => SecretsError::NoKeyring,
            "org.freedesktop.Secret.Error.IsLocked" => SecretsError::Locked,
            _ => SecretsError::failed(error.to_string()),
        },
        other => SecretsError::failed(other.to_string()),
    }
}

fn classify_file(error: file::Error) -> SecretsError {
    match error {
        file::Error::Locked => SecretsError::Locked,
        file::Error::Portal(ashpd::Error::PortalNotFound(_)) => SecretsError::NoKeyring,
        file::Error::Portal(ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled)) => {
            SecretsError::Dismissed
        }
        file::Error::Portal(ashpd::Error::Zbus(error)) => classify_zbus(&error),
        file::Error::Portal(_) => SecretsError::NoKeyring,
        other => SecretsError::failed(other.to_string()),
    }
}

fn to_secret(secret: &oo7::Secret) -> Secret {
    Secret::from_bytes(secret.as_bytes().to_vec())
}

fn to_oo7(secret: Secret) -> oo7::Secret {
    match secret.expose_text() {
        Some(text) => oo7::Secret::text(text),
        None => oo7::Secret::blob(secret.expose_bytes()),
    }
}

impl Backend for Platform {
    fn status(&self) -> BoxFuture<'_, PluginStatus> {
        Box::pin(async move {
            let keyring = match self.keyring().await {
                Ok(keyring) => keyring,
                Err(SecretsError::NoKeyring) => {
                    return PluginStatus::all_unavailable(
                        Flavour::Unsupported,
                        Reason::NoKeyring,
                        NO_KEYRING,
                    )
                }
                Err(error) => {
                    return PluginStatus::all_unavailable(
                        Flavour::Unsupported,
                        Reason::Failed,
                        &error.to_string(),
                    )
                }
            };
            let flavour = match keyring.as_ref() {
                Keyring::File(_) => Flavour::SecretPortal,
                _ => Flavour::SecretService,
            };
            // A locked keyring unlocks when it is used (the desktop asks), so it counts as working until an unlock is refused.
            match keyring.is_locked().await {
                Ok(true) if self.unlock_refused.load(Ordering::Relaxed) => {
                    PluginStatus::all_unavailable(flavour, Reason::Locked, LOCKED)
                }
                Ok(_) => PluginStatus::all_available(flavour),
                Err(error) => {
                    *self.keyring.lock().await = None;
                    match classify(error) {
                        SecretsError::NoKeyring => {
                            PluginStatus::all_unavailable(flavour, Reason::NoKeyring, NO_KEYRING)
                        }
                        other => PluginStatus::all_unavailable(
                            flavour,
                            Reason::Failed,
                            &other.to_string(),
                        ),
                    }
                }
            }
        })
    }

    fn store(
        &self,
        id: SecretId,
        label: Option<String>,
        secret: Secret,
    ) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            let label = label.unwrap_or_else(|| format!("{} ({})", id.service, id.kind.as_str()));
            let attributes = self.attributes_of(&id);
            self.with_keyring(|keyring| async move {
                self.ensure_unlocked(&keyring).await?;
                keyring
                    .create_item(&label, &attributes, to_oo7(secret), true)
                    .await
                    .map_err(classify)
            })
            .await
        })
    }

    fn fetch(&self, id: SecretId) -> BoxFuture<'_, Result<Option<Secret>>> {
        Box::pin(async move {
            let attributes = self.attributes_of(&id);
            self.with_keyring(|keyring| async move {
                self.ensure_unlocked(&keyring).await?;
                let items = keyring.search_items(&attributes).await.map_err(classify)?;
                match items.first() {
                    None => Ok(None),
                    Some(item) => Ok(Some(to_secret(&item.secret().await.map_err(classify)?))),
                }
            })
            .await
        })
    }

    fn exists(&self, id: SecretId) -> BoxFuture<'_, Result<bool>> {
        Box::pin(async move {
            let attributes = self.attributes_of(&id);
            self.with_keyring(|keyring| async move {
                let items = keyring.search_items(&attributes).await.map_err(classify)?;
                Ok(!items.is_empty())
            })
            .await
        })
    }

    fn delete(&self, id: SecretId) -> BoxFuture<'_, Result<bool>> {
        Box::pin(async move {
            let attributes = self.attributes_of(&id);
            self.with_keyring(|keyring| async move {
                self.ensure_unlocked(&keyring).await?;
                let found = !keyring
                    .search_items(&attributes)
                    .await
                    .map_err(classify)?
                    .is_empty();
                if found {
                    keyring.delete(&attributes).await.map_err(classify)?;
                }
                Ok(found)
            })
            .await
        })
    }

    fn delete_account(&self, service: String, account: String) -> BoxFuture<'_, Result<usize>> {
        Box::pin(async move {
            let attributes = self.attributes(&service, &account, None);
            self.with_keyring(|keyring| async move {
                self.ensure_unlocked(&keyring).await?;
                let count = keyring
                    .search_items(&attributes)
                    .await
                    .map_err(classify)?
                    .len();
                if count > 0 {
                    keyring.delete(&attributes).await.map_err(classify)?;
                }
                Ok(count)
            })
            .await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_bus_or_service_is_no_keyring() {
        assert_eq!(
            classify(oo7::Error::DBus(dbus::Error::ZBus(
                oo7::zbus::Error::Address("no bus".into())
            ))),
            SecretsError::NoKeyring
        );
        assert_eq!(
            classify(oo7::Error::DBus(dbus::Error::IO(std::io::Error::other(
                "x"
            )))),
            SecretsError::NoKeyring
        );
        assert_eq!(
            classify(oo7::Error::DBus(dbus::Error::NotFound("default".into()))),
            SecretsError::NoKeyring
        );
    }

    #[test]
    fn a_locked_or_dismissed_keyring_is_told_apart() {
        assert_eq!(
            classify(oo7::Error::DBus(dbus::Error::Dismissed)),
            SecretsError::Dismissed
        );
        assert_eq!(
            classify(oo7::Error::DBus(dbus::Error::Service(
                dbus::ServiceError::IsLocked("/c".into())
            ))),
            SecretsError::Locked
        );
        assert_eq!(
            classify(oo7::Error::File(file::Error::Locked)),
            SecretsError::Locked
        );
    }

    #[test]
    fn an_absent_portal_is_no_keyring() {
        let missing = ashpd::Error::PortalNotFound(
            oo7::zbus::names::OwnedInterfaceName::try_from("org.freedesktop.portal.Secret")
                .unwrap(),
        );
        assert_eq!(
            classify(oo7::Error::File(file::Error::Portal(missing))),
            SecretsError::NoKeyring
        );
    }

    #[test]
    fn attributes_carry_the_namespace_and_the_kind() {
        let platform = Platform::new("org.example.app".into());
        let id = SecretId::new("sftp", "home", crate::models::SecretKind::Passphrase);
        let attributes = platform.attributes_of(&id);
        assert_eq!(attributes["application"], "org.example.app");
        assert_eq!(attributes["service"], "sftp");
        assert_eq!(attributes["account"], "home");
        assert_eq!(attributes["kind"], "passphrase");
        assert!(!platform
            .attributes("sftp", "home", None)
            .contains_key("kind"));
    }
}
