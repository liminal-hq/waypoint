// The providers Waypoint can serve, by URI scheme: one registry shared by the vfs plugin and the
// operations engine, so a scheme exists everywhere or nowhere (A78, A85).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;
use std::sync::Arc;

use waypoint_path::VfsPath;
use waypoint_protocol::{Location, VfsError};

use crate::provider::Provider;

/// Providers by scheme. Registering a second provider for a scheme replaces the first.
#[derive(Clone, Default)]
pub struct ProviderRegistry {
    by_scheme: BTreeMap<&'static str, Arc<dyn Provider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `provider` under its scheme, replacing any earlier one.
    pub fn register(&mut self, provider: Arc<dyn Provider>) {
        self.by_scheme.insert(provider.scheme(), provider);
    }

    /// Whether a provider serves `scheme` (compared ignoring case).
    pub fn serves(&self, scheme: &str) -> bool {
        self.by_scheme
            .keys()
            .any(|known| known.eq_ignore_ascii_case(scheme))
    }

    /// The schemes served, in order.
    pub fn schemes(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.by_scheme.keys().copied()
    }

    /// The provider of `path`, or `Unsupported` naming the scheme.
    pub fn for_path(&self, path: &VfsPath) -> Result<Arc<dyn Provider>, VfsError> {
        self.by_scheme
            .get(path.scheme())
            .cloned()
            .ok_or_else(|| VfsError::Unsupported {
                what: path.scheme().to_owned(),
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
        let mut registry = ProviderRegistry::new();
        registry.register(Arc::new(LocalProvider::new()));
        assert!(registry.serves("file") && registry.serves("FILE"));
        assert!(!registry.serves("sftp"));
        assert_eq!(registry.schemes().collect::<Vec<_>>(), ["file"]);
        let remote = VfsPath::from_uri("sftp://h/a").unwrap();
        assert!(matches!(
            registry.for_path(&remote),
            Err(VfsError::Unsupported { what }) if what == "sftp"
        ));
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
