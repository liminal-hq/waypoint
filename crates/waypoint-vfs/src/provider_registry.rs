// The providers Waypoint can serve, by URI scheme: one registry shared by the vfs plugin and the
// operations engine, so a scheme exists everywhere or nowhere (A78, A85).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, RwLock};

use waypoint_path::VfsPath;
use waypoint_protocol::{Location, VfsError};

use crate::provider::Provider;

#[derive(Default)]
struct Inner {
    by_scheme: BTreeMap<&'static str, Arc<dyn Provider>>,
    /// Schemes the build has a provider for that are turned off, so an address in one is told
    /// apart from a scheme nothing serves (D167).
    off: BTreeSet<&'static str>,
}

/// Providers by scheme. Registering a second provider for a scheme replaces the first. Clones
/// share one table, and providers can be registered and turned off while the app runs (D167), so
/// every holder sees the change at once.
#[derive(Clone, Default)]
pub struct ProviderRegistry {
    inner: Arc<RwLock<Inner>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, Inner> {
        self.inner.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, Inner> {
        self.inner.write().unwrap_or_else(|e| e.into_inner())
    }

    /// Adds `provider` under its scheme, replacing any earlier one, and turns the scheme on.
    pub fn register(&self, provider: Arc<dyn Provider>) {
        let mut inner = self.write();
        inner.off.remove(provider.scheme());
        inner.by_scheme.insert(provider.scheme(), provider);
    }

    /// Takes the provider of `scheme` out and remembers that the scheme is turned off, so its
    /// addresses fail with `ProtocolOff` and not `Unsupported`. Returns the provider that was
    /// serving it, if any.
    pub fn turn_off(&self, scheme: &'static str) -> Option<Arc<dyn Provider>> {
        let mut inner = self.write();
        inner.off.insert(scheme);
        inner.by_scheme.remove(scheme)
    }

    /// Whether `scheme` is a protocol the build has that is turned off (compared ignoring case).
    pub fn is_off(&self, scheme: &str) -> bool {
        self.read()
            .off
            .iter()
            .any(|known| known.eq_ignore_ascii_case(scheme))
    }

    /// The schemes that are turned off, in order.
    pub fn off_schemes(&self) -> Vec<&'static str> {
        self.read().off.iter().copied().collect()
    }

    /// Whether a provider serves `scheme` (compared ignoring case).
    pub fn serves(&self, scheme: &str) -> bool {
        self.read()
            .by_scheme
            .keys()
            .any(|known| known.eq_ignore_ascii_case(scheme))
    }

    /// The schemes served, in order.
    pub fn schemes(&self) -> Vec<&'static str> {
        self.read().by_scheme.keys().copied().collect()
    }

    /// Every provider, in the order of their schemes.
    pub fn providers(&self) -> Vec<Arc<dyn Provider>> {
        self.read().by_scheme.values().cloned().collect()
    }

    /// The provider of `scheme`, if one is registered.
    pub fn get(&self, scheme: &str) -> Option<Arc<dyn Provider>> {
        self.read().by_scheme.get(scheme).cloned()
    }

    /// The provider of `path`, `ProtocolOff` naming a scheme that is turned off, or `Unsupported`
    /// naming a scheme nothing serves.
    pub fn for_path(&self, path: &VfsPath) -> Result<Arc<dyn Provider>, VfsError> {
        self.for_scheme(path.scheme())
    }

    /// The provider of `scheme`, `ProtocolOff` when it is turned off, or `Unsupported` when
    /// nothing serves it.
    pub fn for_scheme(&self, scheme: &str) -> Result<Arc<dyn Provider>, VfsError> {
        let inner = self.read();
        if let Some(provider) = inner.by_scheme.get(scheme) {
            return Ok(provider.clone());
        }
        let scheme = scheme.to_owned();
        Err(if inner.off.contains(scheme.as_str()) {
            VfsError::ProtocolOff { scheme }
        } else {
            VfsError::Unsupported { what: scheme }
        })
    }

    /// Reads a location to its path and finds the provider.
    pub fn for_location(
        &self,
        location: &Location,
    ) -> Result<(VfsPath, Arc<dyn Provider>), VfsError> {
        let path = VfsPath::from_location(location).map_err(|_| VfsError::InvalidLocation {
            input: location.uri.clone(),
        })?;
        let provider = self.for_path(&path)?;
        Ok((path, provider))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LocalProvider;

    #[test]
    fn finds_providers_by_scheme_and_names_what_is_missing() {
        let registry = ProviderRegistry::new();
        registry.register(Arc::new(LocalProvider::new()));
        assert!(registry.serves("file") && registry.serves("FILE"));
        assert!(!registry.serves("sftp"));
        assert_eq!(registry.schemes(), ["file"]);
        let remote = VfsPath::from_uri("sftp://h/a").unwrap();
        assert!(matches!(
            registry.for_path(&remote),
            Err(VfsError::Unsupported { what }) if what == "sftp"
        ));
        // A protocol that is turned off is told apart from one nothing serves, and turning it on
        // again needs no new registry.
        let sftp = Arc::new(crate::FakeRemoteProvider::new(
            waypoint_path::RemoteScheme::Sftp,
            waypoint_path::CaseRule::Sensitive,
        ));
        registry.register(sftp);
        assert!(registry.serves("sftp") && !registry.is_off("sftp"));
        assert!(registry.turn_off("sftp").is_some());
        assert!(!registry.serves("sftp") && registry.is_off("SFTP"));
        assert_eq!(registry.off_schemes(), ["sftp"]);
        assert!(matches!(
            registry.for_path(&remote),
            Err(VfsError::ProtocolOff { scheme }) if scheme == "sftp"
        ));
        assert!(registry.turn_off("sftp").is_none());
        // A local root is a drive on Windows.
        let root = if cfg!(windows) {
            "file:///C:/"
        } else {
            "file:///"
        };
        let local = Location::new("/", root);
        assert_eq!(registry.for_location(&local).unwrap().1.scheme(), "file");
        assert!(matches!(
            registry.for_location(&Location::new("x", "nonsense")),
            Err(VfsError::InvalidLocation { .. })
        ));
    }
}
