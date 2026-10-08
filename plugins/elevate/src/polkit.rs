// Decides whether the polkit route to an administrator helper is usable here, from facts the Probe reports
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};

use crate::models::{Flavour, PluginStatus};
use crate::probe::{FileKind, Probe};
use crate::Config;

/// Where `pkexec` is looked for first; `PATH` is the fallback for systems that keep it elsewhere.
pub const PKEXEC: &str = "/usr/bin/pkexec";

/// Why the plugin is unavailable in a Flatpak sandbox.
pub const REASON_FLATPAK: &str = "Not available in Flatpak";
pub const REASON_APPIMAGE: &str =
    "Elevation is not available in the AppImage; install the .deb or .rpm";
pub const REASON_NO_PKEXEC: &str = "pkexec (polkit) was not found";
pub const REASON_NO_HELPER: &str = "The administrator helper is not installed";
pub const REASON_HELPER_NOT_ROOT: &str = "The administrator helper is not owned by root";
pub const REASON_HELPER_WRITABLE: &str = "The helper is writable by other users";
pub const REASON_FOLDER_UNSAFE: &str =
    "A folder that holds the helper is not owned by root, or is writable by other users";
pub const REASON_NO_POLICY: &str = "The polkit policy is not installed";

/// The result of the check: the status to report, and the `pkexec` to run when it is available.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub status: PluginStatus,
    pub pkexec: Option<PathBuf>,
}

fn unavailable(reason: &str) -> Verdict {
    Verdict {
        status: PluginStatus::unavailable(Flavour::Polkit, reason),
        pkexec: None,
    }
}

/// True when no user but root can change what is at these facts.
fn root_only(owner: u32, mode: u32) -> bool {
    owner == 0 && mode & 0o022 == 0
}

/// Runs the checks in order and stops at the first that fails. The helper is only offered when it, and every folder above it, belongs to root and cannot be changed by anyone else, so a program the person planted can never be what the system's prompt elevates.
pub fn check(config: &Config, probe: &dyn Probe) -> Verdict {
    if probe.facts_following(Path::new("/.flatpak-info")).is_some()
        || probe.env("container").as_deref() == Some("flatpak")
    {
        return unavailable(REASON_FLATPAK);
    }
    if probe.env("APPIMAGE").is_some() || probe.env("APPDIR").is_some() {
        return unavailable(REASON_APPIMAGE);
    }

    let executable = |path: &Path| {
        path.is_absolute()
            && probe
                .facts_following(path)
                .is_some_and(|facts| facts.kind == FileKind::File && facts.mode & 0o111 != 0)
    };
    let fixed = PathBuf::from(PKEXEC);
    let pkexec = if executable(&fixed) {
        Some(fixed)
    } else {
        probe.which("pkexec").filter(|found| executable(found))
    };
    let Some(pkexec) = pkexec else {
        return unavailable(REASON_NO_PKEXEC);
    };

    let Some(helper) = probe.facts(&config.helper) else {
        return unavailable(REASON_NO_HELPER);
    };
    if helper.kind != FileKind::File || !config.helper.is_absolute() {
        return unavailable(REASON_NO_HELPER);
    }
    if helper.uid != 0 {
        return unavailable(REASON_HELPER_NOT_ROOT);
    }
    if helper.mode & 0o022 != 0 {
        return unavailable(REASON_HELPER_WRITABLE);
    }
    for folder in config.helper.ancestors().skip(1) {
        if folder.as_os_str().is_empty() {
            continue;
        }
        let safe = probe.facts_following(folder).is_some_and(|facts| {
            facts.kind == FileKind::Directory && root_only(facts.uid, facts.mode)
        });
        if !safe {
            return unavailable(REASON_FOLDER_UNSAFE);
        }
    }

    if probe
        .facts_following(&config.policy)
        .is_none_or(|facts| facts.kind != FileKind::File)
    {
        return unavailable(REASON_NO_POLICY);
    }

    Verdict {
        status: PluginStatus::available(Flavour::Polkit),
        pkexec: Some(pkexec),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::probe::FileFacts;

    #[derive(Default)]
    struct Fake {
        files: HashMap<PathBuf, FileFacts>,
        env: HashMap<String, String>,
        on_path: Option<PathBuf>,
    }

    impl Fake {
        fn add(&mut self, path: &str, kind: FileKind, uid: u32, mode: u32) {
            self.files
                .insert(PathBuf::from(path), FileFacts { kind, uid, mode });
        }
    }

    impl Probe for Fake {
        fn facts(&self, path: &Path) -> Option<FileFacts> {
            self.files.get(path).copied()
        }
        fn facts_following(&self, path: &Path) -> Option<FileFacts> {
            self.facts(path)
        }
        fn env(&self, name: &str) -> Option<String> {
            self.env.get(name).cloned()
        }
        fn which(&self, _name: &str) -> Option<PathBuf> {
            self.on_path.clone()
        }
    }

    fn config() -> Config {
        Config::new(
            "/usr/libexec/tool/helper",
            "/usr/share/polkit-1/actions/tool.policy",
            "helper ready",
        )
    }

    /// A system where everything is in place.
    fn good() -> Fake {
        let mut fake = Fake::default();
        fake.add("/usr/bin/pkexec", FileKind::File, 0, 0o4755);
        fake.add("/usr/libexec/tool/helper", FileKind::File, 0, 0o755);
        for folder in ["/", "/usr", "/usr/libexec", "/usr/libexec/tool"] {
            fake.add(folder, FileKind::Directory, 0, 0o755);
        }
        fake.add(
            "/usr/share/polkit-1/actions/tool.policy",
            FileKind::File,
            0,
            0o644,
        );
        fake
    }

    fn reason(fake: &Fake) -> Option<String> {
        check(&config(), fake).status.reason
    }

    #[test]
    fn a_complete_installation_is_available_with_the_fixed_pkexec() {
        let verdict = check(&config(), &good());
        assert!(verdict.status.available);
        assert_eq!(verdict.status.reason, None);
        assert_eq!(verdict.status.flavour, Flavour::Polkit);
        assert_eq!(verdict.pkexec, Some(PathBuf::from("/usr/bin/pkexec")));
    }

    #[test]
    fn a_flatpak_is_refused_by_either_sign() {
        let mut fake = good();
        fake.add("/.flatpak-info", FileKind::File, 0, 0o644);
        assert_eq!(reason(&fake).as_deref(), Some(REASON_FLATPAK));
        let mut fake = good();
        fake.env.insert("container".into(), "flatpak".into());
        assert_eq!(reason(&fake).as_deref(), Some(REASON_FLATPAK));
        // Another container is not a Flatpak.
        let mut fake = good();
        fake.env.insert("container".into(), "podman".into());
        assert!(check(&config(), &fake).status.available);
    }

    #[test]
    fn an_appimage_is_refused_by_either_variable() {
        for name in ["APPIMAGE", "APPDIR"] {
            let mut fake = good();
            fake.env.insert(name.into(), "/tmp/mnt".into());
            assert_eq!(reason(&fake).as_deref(), Some(REASON_APPIMAGE));
        }
    }

    #[test]
    fn pkexec_is_found_on_the_path_when_it_is_not_in_the_fixed_place() {
        let mut fake = good();
        fake.files.remove(Path::new("/usr/bin/pkexec"));
        assert_eq!(reason(&fake).as_deref(), Some(REASON_NO_PKEXEC));
        fake.add("/run/wrappers/bin/pkexec", FileKind::File, 0, 0o4755);
        fake.on_path = Some(PathBuf::from("/run/wrappers/bin/pkexec"));
        let verdict = check(&config(), &fake);
        assert!(verdict.status.available);
        assert_eq!(
            verdict.pkexec,
            Some(PathBuf::from("/run/wrappers/bin/pkexec"))
        );
    }

    #[test]
    fn a_pkexec_that_is_not_an_executable_file_is_not_found() {
        let mut fake = good();
        fake.add("/usr/bin/pkexec", FileKind::File, 0, 0o644);
        assert_eq!(reason(&fake).as_deref(), Some(REASON_NO_PKEXEC));
        fake.add("/usr/bin/pkexec", FileKind::Directory, 0, 0o755);
        fake.on_path = Some(PathBuf::from("relative/pkexec"));
        assert_eq!(reason(&fake).as_deref(), Some(REASON_NO_PKEXEC));
    }

    #[test]
    fn a_missing_or_irregular_helper_is_not_installed() {
        let mut fake = good();
        fake.files.remove(Path::new("/usr/libexec/tool/helper"));
        assert_eq!(reason(&fake).as_deref(), Some(REASON_NO_HELPER));
        fake.add("/usr/libexec/tool/helper", FileKind::Directory, 0, 0o755);
        assert_eq!(reason(&fake).as_deref(), Some(REASON_NO_HELPER));
        fake.add("/usr/libexec/tool/helper", FileKind::Other, 0, 0o755);
        assert_eq!(reason(&fake).as_deref(), Some(REASON_NO_HELPER));
    }

    #[test]
    fn a_helper_that_root_does_not_own_is_refused() {
        let mut fake = good();
        fake.add("/usr/libexec/tool/helper", FileKind::File, 1000, 0o755);
        assert_eq!(reason(&fake).as_deref(), Some(REASON_HELPER_NOT_ROOT));
    }

    #[test]
    fn a_helper_writable_by_group_or_others_is_refused() {
        for mode in [0o775, 0o757, 0o777, 0o4777] {
            let mut fake = good();
            fake.add("/usr/libexec/tool/helper", FileKind::File, 0, mode);
            assert_eq!(
                reason(&fake).as_deref(),
                Some(REASON_HELPER_WRITABLE),
                "{mode:o}"
            );
        }
    }

    #[test]
    fn every_folder_above_the_helper_must_be_root_only() {
        for folder in ["/", "/usr", "/usr/libexec", "/usr/libexec/tool"] {
            let mut fake = good();
            fake.add(folder, FileKind::Directory, 1000, 0o755);
            assert_eq!(
                reason(&fake).as_deref(),
                Some(REASON_FOLDER_UNSAFE),
                "owner of {folder}"
            );
            let mut fake = good();
            fake.add(folder, FileKind::Directory, 0, 0o1777);
            assert_eq!(
                reason(&fake).as_deref(),
                Some(REASON_FOLDER_UNSAFE),
                "mode of {folder}"
            );
            let mut fake = good();
            fake.add(folder, FileKind::Directory, 0, 0o775);
            assert_eq!(
                reason(&fake).as_deref(),
                Some(REASON_FOLDER_UNSAFE),
                "group of {folder}"
            );
        }
        let mut fake = good();
        fake.files.remove(Path::new("/usr/libexec"));
        assert_eq!(reason(&fake).as_deref(), Some(REASON_FOLDER_UNSAFE));
    }

    #[test]
    fn a_relative_helper_path_is_not_installed() {
        let mut fake = good();
        fake.add("tool/helper", FileKind::File, 0, 0o755);
        let mut relative = config();
        relative.helper = PathBuf::from("tool/helper");
        assert_eq!(
            check(&relative, &fake).status.reason.as_deref(),
            Some(REASON_NO_HELPER)
        );
    }

    #[test]
    fn a_missing_policy_is_refused() {
        let mut fake = good();
        fake.files
            .remove(Path::new("/usr/share/polkit-1/actions/tool.policy"));
        assert_eq!(reason(&fake).as_deref(), Some(REASON_NO_POLICY));
    }

    #[test]
    fn the_first_failing_check_names_the_reason() {
        // Everything is wrong; the sandbox comes first, then pkexec, then the helper, then the policy.
        let mut fake = Fake::default();
        fake.env.insert("container".into(), "flatpak".into());
        assert_eq!(reason(&fake).as_deref(), Some(REASON_FLATPAK));
        fake.env.clear();
        assert_eq!(reason(&fake).as_deref(), Some(REASON_NO_PKEXEC));
        fake.add("/usr/bin/pkexec", FileKind::File, 0, 0o4755);
        assert_eq!(reason(&fake).as_deref(), Some(REASON_NO_HELPER));
    }

    #[test]
    fn no_reason_names_a_path() {
        for reason in [
            REASON_FLATPAK,
            REASON_APPIMAGE,
            REASON_NO_PKEXEC,
            REASON_NO_HELPER,
            REASON_HELPER_NOT_ROOT,
            REASON_HELPER_WRITABLE,
            REASON_FOLDER_UNSAFE,
            REASON_NO_POLICY,
        ] {
            assert!(!reason.contains('/'), "{reason}");
        }
    }
}
