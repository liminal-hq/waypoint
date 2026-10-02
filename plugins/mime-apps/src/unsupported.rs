// Reports every feature unavailable on systems with no support for types and applications
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg_attr(any(target_os = "linux", target_os = "windows"), allow(dead_code))]

use crate::backend::{Backend, ParentWindow};
use crate::error::{MimeAppsError, Result};
use crate::models::{Flavour, Handlers, PluginStatus, Reason, TypeInfo};
use crate::target::Target;

const MESSAGE: &str = "this system has no support for file types and their applications";

pub struct Platform;

impl Default for Platform {
    fn default() -> Self {
        Self::new()
    }
}

impl Platform {
    pub fn new() -> Self {
        Platform
    }

    pub fn for_app<R: tauri::Runtime>(_app: &tauri::AppHandle<R>) -> Self {
        Platform
    }
}

impl Backend for Platform {
    fn status(&self) -> PluginStatus {
        PluginStatus::all_unavailable(Flavour::Unsupported, Reason::UnsupportedPlatform, MESSAGE)
    }

    fn type_info(&self, _target: &Target, _sniff: bool) -> Result<TypeInfo> {
        Err(MimeAppsError::Unsupported)
    }

    fn handlers(&self, _targets: &[Target]) -> Result<Handlers> {
        Err(MimeAppsError::Unsupported)
    }

    fn open_with(&self, _targets: &[Target], _app_id: &str) -> Result<()> {
        Err(MimeAppsError::Unsupported)
    }

    fn open_default(&self, _targets: &[Target]) -> Result<()> {
        Err(MimeAppsError::Unsupported)
    }

    fn choose(&self, _targets: &[Target], _parent: ParentWindow) -> Result<()> {
        Err(MimeAppsError::Unsupported)
    }

    fn set_default(&self, _mime: &str, _app_id: &str) -> Result<()> {
        Err(MimeAppsError::Unsupported)
    }

    fn open_default_apps_settings(&self) -> Result<()> {
        Err(MimeAppsError::Unsupported)
    }

    fn knows_app(&self, _app_id: &str) -> bool {
        false
    }

    fn app_icon(&self, _app_id: &str, _size: u32) -> Option<Vec<u8>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::FEATURES;
    use crate::target::parse;

    #[test]
    fn every_feature_is_unavailable_with_a_reason() {
        let status = Platform::new().status();
        assert!(!status.available);
        assert_eq!(status.flavour, Flavour::Unsupported);
        assert_eq!(status.reason, Some(Reason::UnsupportedPlatform));
        let names: Vec<&str> = status.features.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, FEATURES);
        assert!(status.features.iter().all(|f| !f.available
            && f.reason == Some(Reason::UnsupportedPlatform)
            && f.message.is_some()));
    }

    #[test]
    fn every_action_is_unsupported() {
        let platform = Platform::new();
        let target = parse("/a/b.txt").unwrap();
        assert_eq!(
            platform.type_info(&target, false),
            Err(MimeAppsError::Unsupported)
        );
        assert_eq!(platform.handlers(&[]), Err(MimeAppsError::Unsupported));
        assert_eq!(
            platform.open_with(&[], "x"),
            Err(MimeAppsError::Unsupported)
        );
        assert_eq!(platform.open_default(&[]), Err(MimeAppsError::Unsupported));
        assert_eq!(platform.choose(&[], None), Err(MimeAppsError::Unsupported));
        assert_eq!(
            platform.set_default("text/plain", "x"),
            Err(MimeAppsError::Unsupported)
        );
        assert_eq!(
            platform.open_default_apps_settings(),
            Err(MimeAppsError::Unsupported)
        );
        assert!(!platform.knows_app("x"));
        assert_eq!(platform.app_icon("x", 32), None);
    }
}
