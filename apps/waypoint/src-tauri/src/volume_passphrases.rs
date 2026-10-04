// Composes the volumes plugin's passphrase store over the secrets plugin and the Settings switch
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `tauri-plugin-volumes` keeps no passphrase and calls no other plugin (A4): it is handed a
// `PassphraseStore`, and this is it. A passphrase is kept in the keyring as a `passphrase` secret of
// the service `volume`, under the UUID of the encrypted container. The switch on the Integrations
// page (`rememberVolumePassphrases`, off until enabled, D153) decides whether anything is offered,
// kept or used; while it is off nothing unlocks by itself, but what is already in the keyring can
// still be forgotten.

use std::sync::{Arc, OnceLock};

use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_secrets::{Secret, SecretId, SecretKind, Secrets, SecretsError, FEATURE_STORE};
use tauri_plugin_volumes::{BoxFuture, Passphrase, PassphraseStore, Reason, Unavailable};

/// The keyring service the passphrases are filed under.
const SERVICE: &str = "volume";

/// Where the store finds the keyring and the switch; replaced by fakes in tests.
struct Source {
    secrets: Box<dyn Fn() -> Option<Secrets> + Send + Sync>,
    enabled: Box<dyn Fn() -> bool + Send + Sync>,
}

/// The passphrase store the app gives the volumes plugin.
pub struct VolumePassphrases {
    source: Source,
}

/// The store, and the handle that lets it reach the app once there is one: the plugin is built
/// before the app exists.
pub fn store() -> (Arc<VolumePassphrases>, Arc<OnceLock<AppHandle<Wry>>>) {
    let handle: Arc<OnceLock<AppHandle<Wry>>> = Arc::new(OnceLock::new());
    let for_secrets = Arc::clone(&handle);
    let for_switch = Arc::clone(&handle);
    let store = Arc::new(VolumePassphrases {
        source: Source {
            secrets: Box::new(move || {
                for_secrets
                    .get()
                    .and_then(|app| app.try_state::<Secrets>())
                    .map(|secrets| secrets.inner().clone())
            }),
            enabled: Box::new(move || {
                for_switch.get().is_some_and(|app| {
                    crate::settings::current(app)
                        .integrations
                        .remember_volume_passphrases
                })
            }),
        },
    });
    (store, handle)
}

fn id(uuid: &str) -> SecretId {
    SecretId::new(SERVICE, uuid, SecretKind::Passphrase)
}

/// What a keyring failure means to the person: a code the page translates, and an English sentence.
fn unavailable(error: &SecretsError) -> Unavailable {
    match error {
        SecretsError::Locked | SecretsError::Dismissed => {
            Unavailable::new(Reason::KeyringLocked, "The keyring is locked.")
        }
        SecretsError::NoKeyring | SecretsError::Unsupported => {
            Unavailable::new(Reason::NoKeyring, "No keyring is running.")
        }
        other => Unavailable::new(Reason::NoKeyring, other.to_string()),
    }
}

impl VolumePassphrases {
    fn secrets(&self) -> Result<Secrets, Unavailable> {
        (self.source.secrets)()
            .ok_or_else(|| Unavailable::new(Reason::NoKeyring, "No keyring is running."))
    }

    /// The reason to do nothing, or `None` when the switch is on and the keyring answers.
    async fn gate(&self) -> Option<Unavailable> {
        if !(self.source.enabled)() {
            return Some(Unavailable::new(
                Reason::Disabled,
                "Remembering passphrases is turned off in Settings.",
            ));
        }
        let secrets = match self.secrets() {
            Ok(secrets) => secrets,
            Err(why) => return Some(why),
        };
        let status = secrets.get_status().await;
        if status.has(FEATURE_STORE) {
            return None;
        }
        let message = status.message.unwrap_or_default();
        Some(Unavailable::new(
            match status.reason {
                Some(tauri_plugin_secrets::Reason::Locked) => Reason::KeyringLocked,
                _ => Reason::NoKeyring,
            },
            if message.is_empty() {
                "No keyring is running.".to_string()
            } else {
                message
            },
        ))
    }
}

impl PassphraseStore for VolumePassphrases {
    fn status(&self) -> BoxFuture<'_, Option<Unavailable>> {
        Box::pin(self.gate())
    }

    fn remember(
        &self,
        uuid: String,
        passphrase: Passphrase,
    ) -> BoxFuture<'_, Result<(), Unavailable>> {
        Box::pin(async move {
            if let Some(why) = self.gate().await {
                return Err(why);
            }
            self.secrets()?
                .store(
                    &id(&uuid),
                    Some("Encrypted volume passphrase".to_string()),
                    Secret::from_text(passphrase.0),
                )
                .await
                .map_err(|error| unavailable(&error))
        })
    }

    fn recall(&self, uuid: String) -> BoxFuture<'_, Result<Option<Passphrase>, Unavailable>> {
        Box::pin(async move {
            if let Some(why) = self.gate().await {
                return Err(why);
            }
            let found = self
                .secrets()?
                .fetch(&id(&uuid))
                .await
                .map_err(|error| unavailable(&error))?;
            Ok(found
                .and_then(|secret| secret.expose_text().map(str::to_string))
                .map(Passphrase))
        })
    }

    fn has(&self, uuid: String) -> BoxFuture<'_, Result<bool, Unavailable>> {
        Box::pin(async move {
            if let Some(why) = self.gate().await {
                return Err(why);
            }
            self.secrets()?
                .exists(&id(&uuid))
                .await
                .map_err(|error| unavailable(&error))
        })
    }

    fn forget(&self, uuid: String) -> BoxFuture<'_, Result<bool, Unavailable>> {
        // Not gated on the switch: turning remembering off must not strand what was kept.
        Box::pin(async move {
            self.secrets()?
                .delete(&id(&uuid))
                .await
                .map_err(|error| unavailable(&error))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tauri_plugin_secrets::MemoryBackend;

    struct Fixture {
        store: VolumePassphrases,
        backend: Arc<MemoryBackend>,
        enabled: Arc<AtomicBool>,
    }

    fn fixture(enabled: bool) -> Fixture {
        let backend = Arc::new(MemoryBackend::new());
        let secrets = Secrets::new(backend.clone());
        let switch = Arc::new(AtomicBool::new(enabled));
        let reads = Arc::clone(&switch);
        Fixture {
            store: VolumePassphrases {
                source: Source {
                    secrets: Box::new(move || Some(secrets.clone())),
                    enabled: Box::new(move || reads.load(Ordering::SeqCst)),
                },
            },
            backend,
            enabled: switch,
        }
    }

    #[tokio::test]
    async fn a_passphrase_round_trips_under_the_volume_service_and_the_uuid() {
        let fx = fixture(true);
        assert_eq!(fx.store.status().await, None);
        fx.store
            .remember("uuid-1".into(), Passphrase("hunter2".into()))
            .await
            .unwrap();
        assert_eq!(fx.backend.len(), 1);
        assert!(fx
            .backend
            .label_of(&SecretId::new("volume", "uuid-1", SecretKind::Passphrase))
            .is_some());
        assert_eq!(fx.store.has("uuid-1".into()).await, Ok(true));
        assert_eq!(
            fx.store.recall("uuid-1".into()).await.unwrap().unwrap().0,
            "hunter2"
        );
        assert!(fx.store.recall("other".into()).await.unwrap().is_none());
        assert_eq!(fx.store.forget("uuid-1".into()).await, Ok(true));
        assert_eq!(fx.store.has("uuid-1".into()).await, Ok(false));
    }

    #[tokio::test]
    async fn the_switch_off_keeps_and_uses_nothing_but_still_forgets() {
        let fx = fixture(true);
        fx.store
            .remember("uuid-1".into(), Passphrase("hunter2".into()))
            .await
            .unwrap();
        fx.enabled.store(false, Ordering::SeqCst);
        let off = fx.store.status().await.expect("off says why");
        assert_eq!(off.reason, Reason::Disabled);
        assert!(fx
            .store
            .remember("uuid-2".into(), Passphrase("x".into()))
            .await
            .is_err());
        assert_eq!(
            fx.store.recall("uuid-1".into()).await.unwrap_err().reason,
            Reason::Disabled
        );
        assert_eq!(fx.backend.len(), 1, "what was kept stays in the keyring");
        assert_eq!(fx.store.forget("uuid-1".into()).await, Ok(true));
        assert!(fx.backend.is_empty());
    }

    #[tokio::test]
    async fn a_missing_or_locked_keyring_is_a_reason_not_a_failure_of_the_app() {
        let fx = fixture(true);
        fx.backend.fail_with(Some(SecretsError::NoKeyring));
        assert_eq!(fx.store.status().await.unwrap().reason, Reason::NoKeyring);
        fx.backend.fail_with(Some(SecretsError::Locked));
        assert_eq!(
            fx.store.status().await.unwrap().reason,
            Reason::KeyringLocked
        );
        assert_eq!(
            fx.store.forget("uuid-1".into()).await.unwrap_err().reason,
            Reason::KeyringLocked
        );
    }

    #[tokio::test]
    async fn before_the_app_exists_there_is_no_keyring() {
        let (store, _handle) = store();
        let why = store.status().await.expect("not ready");
        assert_eq!(why.reason, Reason::Disabled);
    }
}
