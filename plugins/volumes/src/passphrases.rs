// The seam to wherever the host keeps passphrases: what the plugin needs to remember and recall the passphrase of an encrypted volume
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use crate::backend::BoxFuture;
use crate::models::{Passphrase, Reason};

/// Why passphrases cannot be kept or read right now: a code to branch on and a sentence for people.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unavailable {
    pub reason: Reason,
    pub message: String,
}

impl Unavailable {
    pub fn new(reason: Reason, message: impl Into<String>) -> Self {
        Unavailable {
            reason,
            message: message.into(),
        }
    }
}

/// The host's place for passphrases, keyed by the UUID of an encrypted volume's container. The plugin knows nothing of what is behind it: the host decides whether remembering is on at all (a setting) and where the secrets go (a keyring), and plugins never call each other, so the host implements this over whatever it has. An implementation must never log or write a passphrase anywhere but its keyring.
pub trait PassphraseStore: Send + Sync + 'static {
    /// `None` when passphrases can be kept and read now; otherwise why not. Never prompts. The plugin hides what it offers (and does nothing by itself) while this says no.
    fn status(&self) -> BoxFuture<'_, Option<Unavailable>>;

    /// Keeps the passphrase of the volume whose container has this UUID, replacing one.
    fn remember(
        &self,
        uuid: String,
        passphrase: Passphrase,
    ) -> BoxFuture<'_, Result<(), Unavailable>>;

    /// The passphrase kept for this UUID, if any.
    fn recall(&self, uuid: String) -> BoxFuture<'_, Result<Option<Passphrase>, Unavailable>>;

    /// Whether one is kept, without reading it.
    fn has(&self, uuid: String) -> BoxFuture<'_, Result<bool, Unavailable>>;

    /// Forgets it; true when there was one.
    fn forget(&self, uuid: String) -> BoxFuture<'_, Result<bool, Unavailable>>;
}

pub type SharedStore = Arc<dyn PassphraseStore>;
