// Decides whether the UAC route to an administrator helper is usable here, from facts a WinProbe reports
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::Path;

use crate::location::{is_under, normalise};
use crate::models::{Flavour, PluginStatus};
use crate::Config;

pub const REASON_ALREADY_ADMIN: &str = "The app is already running as an administrator";
pub const REASON_PORTABLE: &str =
    "Elevation is not available in the portable version; install the app with its installer";
pub const REASON_NO_FOLDERS: &str = "The Program Files folder could not be found";
pub const REASON_NOT_CONFIGURED: &str = "Elevation is not set up in this build";
pub const REASON_NO_HELPER: &str = "The administrator helper is not installed";
pub const REASON_HELPER_LINK: &str = "The administrator helper is not a regular file";

/// What kind of thing is at the helper's path, as the check cares. A link or other reparse point is never trusted, because it can name a file the person can change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HelperFacts {
    pub is_file: bool,
    pub is_reparse_point: bool,
}

/// The facts about the system that the check reads. The real one asks Windows; tests substitute a fake.
pub trait WinProbe {
    /// The facts about `path` without following a final link, or `None` when nothing is there or it cannot be read.
    fn helper_facts(&self, path: &Path) -> Option<HelperFacts>;
    /// The Program Files folders Windows knows (native, 32-bit and 64-bit), as text. Standard users cannot write to them.
    fn program_files(&self) -> Vec<String>;
    /// The path of the running app, as text.
    fn app_exe(&self) -> Option<String>;
    /// Whether this process is running elevated, when that can be read.
    fn is_elevated(&self) -> Option<bool>;
}

fn unavailable(reason: &str) -> PluginStatus {
    PluginStatus::unavailable(Flavour::Uac, reason)
}

/// Runs the checks in order and stops at the first that fails. The helper is offered only when both it and the app sit under a Program Files folder, where a standard user cannot replace them, so a program the person planted can never be what the prompt elevates; the portable copy cannot satisfy that.
pub fn check(config: &Config, probe: &dyn WinProbe) -> PluginStatus {
    if probe.is_elevated() == Some(true) {
        return unavailable(REASON_ALREADY_ADMIN);
    }

    if config.pipe_prefix.is_none() {
        return unavailable(REASON_NOT_CONFIGURED);
    }

    let folders = probe.program_files();
    if folders.iter().all(|folder| normalise(folder).is_none()) {
        return unavailable(REASON_NO_FOLDERS);
    }
    let protected = |path: &str| folders.iter().any(|folder| is_under(path, folder));
    let helper_text = config.helper.to_string_lossy();
    let app_protected = probe.app_exe().is_some_and(|app| protected(&app));
    if !protected(&helper_text) || !app_protected {
        return unavailable(REASON_PORTABLE);
    }

    match probe.helper_facts(&config.helper) {
        None => unavailable(REASON_NO_HELPER),
        Some(facts) if facts.is_reparse_point => unavailable(REASON_HELPER_LINK),
        Some(facts) if !facts.is_file => unavailable(REASON_NO_HELPER),
        Some(_) => PluginStatus::available(Flavour::Uac),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake {
        facts: Option<HelperFacts>,
        folders: Vec<String>,
        app: Option<String>,
        elevated: Option<bool>,
    }

    impl Fake {
        fn installed() -> Self {
            Fake {
                facts: Some(HelperFacts {
                    is_file: true,
                    is_reparse_point: false,
                }),
                folders: vec![r"C:\Program Files".into(), r"C:\Program Files (x86)".into()],
                app: Some(r"C:\Program Files\Tool\tool.exe".into()),
                elevated: Some(false),
            }
        }
    }

    impl WinProbe for Fake {
        fn helper_facts(&self, _: &Path) -> Option<HelperFacts> {
            self.facts
        }
        fn program_files(&self) -> Vec<String> {
            self.folders.clone()
        }
        fn app_exe(&self) -> Option<String> {
            self.app.clone()
        }
        fn is_elevated(&self) -> Option<bool> {
            self.elevated
        }
    }

    fn config() -> Config {
        Config::new(r"C:\Program Files\Tool\helper.exe", "", "ready")
            .with_pipe_prefix("tool-elevate")
    }

    fn reason(probe: &Fake) -> Option<String> {
        check(&config(), probe).reason
    }

    #[test]
    fn an_installed_helper_under_program_files_is_available() {
        let status = check(&config(), &Fake::installed());
        assert!(status.available, "{:?}", status.reason);
        assert_eq!(status.flavour, Flavour::Uac);
    }

    #[test]
    fn the_x86_folder_counts_too() {
        let mut probe = Fake::installed();
        probe.app = Some(r"C:\Program Files (x86)\Tool\tool.exe".into());
        let config = Config::new(r"C:\Program Files (x86)\Tool\helper.exe", "", "ready")
            .with_pipe_prefix("p");
        assert!(check(&config, &probe).available);
    }

    #[test]
    fn a_process_that_is_already_an_administrator_has_nothing_to_elevate() {
        let mut probe = Fake::installed();
        probe.elevated = Some(true);
        assert_eq!(reason(&probe).as_deref(), Some(REASON_ALREADY_ADMIN));
    }

    #[test]
    fn an_elevation_state_that_cannot_be_read_does_not_block() {
        let mut probe = Fake::installed();
        probe.elevated = None;
        assert!(check(&config(), &probe).available);
    }

    #[test]
    fn a_build_without_a_pipe_prefix_cannot_launch() {
        let config = Config::new(r"C:\Program Files\Tool\helper.exe", "", "ready");
        assert_eq!(
            check(&config, &Fake::installed()).reason.as_deref(),
            Some(REASON_NOT_CONFIGURED)
        );
    }

    #[test]
    fn a_copy_outside_program_files_is_the_portable_version() {
        let mut probe = Fake::installed();
        let config = Config::new(r"C:\Users\me\Downloads\Tool\helper.exe", "", "ready")
            .with_pipe_prefix("p");
        assert_eq!(
            check(&config, &probe).reason.as_deref(),
            Some(REASON_PORTABLE)
        );
        // The app outside, the helper inside, is still not an installation.
        probe.app = Some(r"C:\Users\me\Tool\tool.exe".into());
        assert_eq!(reason(&probe).as_deref(), Some(REASON_PORTABLE));
        probe.app = None;
        assert_eq!(reason(&probe).as_deref(), Some(REASON_PORTABLE));
    }

    #[test]
    fn a_helper_path_that_climbs_out_of_program_files_is_refused() {
        let probe = Fake::installed();
        let config = Config::new(r"C:\Program Files\..\Users\me\helper.exe", "", "ready")
            .with_pipe_prefix("p");
        assert_eq!(
            check(&config, &probe).reason.as_deref(),
            Some(REASON_PORTABLE)
        );
    }

    #[test]
    fn unknown_program_files_folders_fail_closed() {
        let mut probe = Fake::installed();
        probe.folders = vec![];
        assert_eq!(reason(&probe).as_deref(), Some(REASON_NO_FOLDERS));
        probe.folders = vec!["not a path".into()];
        assert_eq!(reason(&probe).as_deref(), Some(REASON_NO_FOLDERS));
    }

    #[test]
    fn a_missing_helper_is_not_installed() {
        let mut probe = Fake::installed();
        probe.facts = None;
        assert_eq!(reason(&probe).as_deref(), Some(REASON_NO_HELPER));
        probe.facts = Some(HelperFacts {
            is_file: false,
            is_reparse_point: false,
        });
        assert_eq!(reason(&probe).as_deref(), Some(REASON_NO_HELPER));
    }

    #[test]
    fn a_link_in_place_of_the_helper_is_refused() {
        let mut probe = Fake::installed();
        probe.facts = Some(HelperFacts {
            is_file: true,
            is_reparse_point: true,
        });
        assert_eq!(reason(&probe).as_deref(), Some(REASON_HELPER_LINK));
    }
}
