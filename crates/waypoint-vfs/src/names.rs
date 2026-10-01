// Checks that a name can be created or renamed to, under a provider's case rule.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::OsStr;

use waypoint_path::{windows, CaseRule, VfsPath};
use waypoint_protocol::VfsError;

/// The longest name, in bytes on a case-sensitive (Linux) provider and in UTF-16 units on a
/// case-insensitive (Windows) one.
const MAX_NAME: usize = 255;

fn invalid(name: &OsStr, reason: &str) -> VfsError {
    VfsError::InvalidName {
        name: name.to_string_lossy().into_owned(),
        reason: reason.to_owned(),
    }
}

/// Whether `name` can be the name of a new entry. A case-sensitive provider follows the POSIX rules
/// (no empty name, `.` or `..`, separator, NUL; at most 255 bytes); a case-insensitive one follows
/// the Windows rules besides (no reserved device names, no `<>:"|?*` or control characters, no
/// trailing dot or space; at most 255 UTF-16 units). The rule is chosen by `CaseRule` because that
/// is what tells a provider's file system apart.
pub fn validate_name(name: &OsStr, rule: CaseRule) -> Result<(), VfsError> {
    let text = name.to_string_lossy();
    if name.is_empty() {
        return Err(invalid(name, "a name cannot be empty"));
    }
    if text == "." || text == ".." {
        return Err(invalid(name, "a name cannot be `.` or `..`"));
    }
    if text.contains('\0') {
        return Err(invalid(name, "a name cannot contain a NUL character"));
    }
    if text.contains('/') {
        return Err(invalid(name, "a name cannot contain `/`"));
    }
    match rule {
        CaseRule::Sensitive => {
            if name.as_encoded_bytes().len() > MAX_NAME {
                return Err(invalid(name, "a name can be at most 255 bytes"));
            }
        }
        CaseRule::Insensitive => {
            if text.encode_utf16().count() > MAX_NAME {
                return Err(invalid(name, "a name can be at most 255 characters"));
            }
            if text.contains('\\') {
                return Err(invalid(name, "a name cannot contain `\\`"));
            }
            if let Some(c) = text
                .chars()
                .find(|c| c.is_control() || "<>:\"|?*".contains(*c))
            {
                let shown: String = c.escape_default().collect();
                return Err(invalid(
                    name,
                    &format!("a name cannot contain `{shown}` on Windows"),
                ));
            }
            if text.ends_with(['.', ' ']) {
                return Err(invalid(name, "a name cannot end with a dot or a space"));
            }
            if windows::is_reserved_name(&text) {
                return Err(invalid(name, "that name is reserved by Windows"));
            }
        }
    }
    Ok(())
}

/// The path of a new child called `name` in `parent`, after checking the name. This is how a caller
/// turns a typed or computed name into a path: `VfsPath::join` would resolve `..` and separators
/// instead of refusing them.
pub fn child_path(parent: &VfsPath, name: &OsStr, rule: CaseRule) -> Result<VfsPath, VfsError> {
    validate_name(name, rule)?;
    parent
        .join(name)
        .map_err(|_| invalid(name, "not a valid name"))
}

/// Checks the final component of a path about to be created or renamed to.
pub(crate) fn validate_new_path(path: &VfsPath, rule: CaseRule) -> Result<(), VfsError> {
    let VfsPath::File(file) = path;
    match file.file_name() {
        Some(name) => validate_name(&name, rule),
        None => Err(VfsError::InvalidName {
            name: path.display(),
            reason: "a root has no name to create".to_owned(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bad(name: &str, rule: CaseRule) {
        assert!(
            matches!(
                validate_name(OsStr::new(name), rule),
                Err(VfsError::InvalidName { .. })
            ),
            "{name:?} should be refused under {rule:?}"
        );
    }

    fn good(name: &str, rule: CaseRule) {
        assert_eq!(
            validate_name(OsStr::new(name), rule),
            Ok(()),
            "{name:?} under {rule:?}"
        );
    }

    #[test]
    fn both_rules_refuse_the_structural_names() {
        for rule in [CaseRule::Sensitive, CaseRule::Insensitive] {
            for name in ["", ".", "..", "a/b", "a\0b", "/"] {
                bad(name, rule);
            }
            bad(&"x".repeat(256), rule);
            good("notes.txt", rule);
            good(".hidden", rule);
            good("a b", rule);
            good("...x", rule);
            good(&"x".repeat(255), rule);
        }
    }

    #[test]
    fn linux_allows_what_windows_refuses() {
        for name in [
            "con",
            "NUL.txt",
            "a:b",
            "trailing.",
            "trailing ",
            "a\\b",
            "q?",
            "t\tab",
        ] {
            good(name, CaseRule::Sensitive);
            bad(name, CaseRule::Insensitive);
        }
        for name in ["COM1", "lpt9", "Aux.tar.gz", "com1 "] {
            bad(name, CaseRule::Insensitive);
        }
        good("COM10", CaseRule::Insensitive);
        good("console", CaseRule::Insensitive);
    }

    #[test]
    fn length_counts_bytes_on_linux_and_utf16_units_on_windows() {
        let name = "é".repeat(128); // 256 bytes, 128 units
        bad(&name, CaseRule::Sensitive);
        good(&name, CaseRule::Insensitive);
    }

    #[test]
    fn a_child_path_joins_only_a_checked_name() {
        let parent =
            VfsPath::File(waypoint_path::FilePath::from_path(std::env::temp_dir()).unwrap());
        let child = child_path(&parent, OsStr::new("new"), CaseRule::NATIVE).unwrap();
        assert_eq!(child.parent().unwrap(), parent);
        assert!(child_path(&parent, OsStr::new("../escape"), CaseRule::NATIVE).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn a_root_cannot_be_created() {
        let root = VfsPath::File(waypoint_path::FilePath::parse("/").unwrap());
        assert!(matches!(
            validate_new_path(&root, CaseRule::NATIVE),
            Err(VfsError::InvalidName { .. })
        ));
    }
}
