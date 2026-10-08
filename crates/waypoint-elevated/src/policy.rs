// What the helper checks about a request before it touches the file system.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::{OsStr, OsString};
use std::path::{Component, Path};

use waypoint_path::{FilePath, VfsPath};
use waypoint_protocol::VfsError;

use crate::os_name::WireOs;

fn refuse(why: &str) -> VfsError {
    // The reason names the rule, never the path: the answer travels back over the wire, and
    // nothing here may carry a path into a log.
    VfsError::InvalidLocation {
        input: why.to_owned(),
    }
}

/// Reads a path off the wire. The helper acts only on an absolute path in the form `FilePath` gives
/// it: no `.` or `..`, no repeated or trailing separators. The client normalises every path before
/// sending, so a path that is not already in that form is not from a well-behaved client and is
/// refused rather than tidied. What comes back is the `file:` path the helper's own provider uses;
/// the wire has no other scheme to name.
pub fn path(wire: &WireOs) -> Result<VfsPath, VfsError> {
    let raw: OsString = wire
        .to_os_string()
        .map_err(|_| refuse("the path is not in this platform's form"))?;
    if raw.is_empty() {
        return Err(refuse("the path is empty"));
    }
    let candidate = Path::new(&raw);
    if !candidate.is_absolute() {
        return Err(refuse("the path is not absolute"));
    }
    if candidate
        .components()
        .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
    {
        return Err(refuse("the path has a `.` or `..` part"));
    }
    let file = FilePath::from_path(candidate).map_err(|_| refuse("the path is not valid"))?;
    if file.as_path().as_os_str() != raw.as_os_str() {
        return Err(refuse("the path is not in its normal form"));
    }
    Ok(VfsPath::File(file))
}

/// Whether `name` is one name: not empty, not `.` or `..`, with no separator and no NUL. A name
/// the helper is handed (or hands back) is joined onto a folder, so one that is more than a single
/// component would reach elsewhere.
pub fn name_ok(name: &OsStr) -> bool {
    if name.is_empty() || name.as_encoded_bytes().contains(&0) {
        return false;
    }
    let mut parts = Path::new(name).components();
    matches!(
        (parts.next(), parts.next()),
        (Some(Component::Normal(only)), None) if only == name
    )
}

/// Reads a name off the wire.
pub fn name(wire: &WireOs) -> Result<OsString, VfsError> {
    let name = wire
        .to_os_string()
        .map_err(|_| refuse("the name is not in this platform's form"))?;
    if !name_ok(&name) {
        return Err(VfsError::InvalidName {
            name: String::new(),
            reason: "not a single file name".to_owned(),
        });
    }
    Ok(name)
}

/// Whether one more of something that is held in a table may be admitted: `held` is how many there
/// are now. Refusing is an answer, not a closed connection, so a client that hits the limit can
/// close something and carry on.
pub fn admit(held: usize, limit: usize, what: &str) -> Result<(), VfsError> {
    if held >= limit {
        return Err(VfsError::Io {
            message: format!("too many {what}"),
            location: None,
        });
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn text(path: &str) -> WireOs {
        WireOs::Text(path.to_owned())
    }

    fn kind(result: Result<VfsPath, VfsError>) -> String {
        match result {
            Ok(path) => format!("ok {}", path.display()),
            Err(VfsError::InvalidLocation { .. }) => "invalidLocation".to_owned(),
            Err(other) => format!("{other:?}"),
        }
    }

    #[test]
    fn an_absolute_normal_path_is_accepted_as_a_file_path() {
        assert_eq!(kind(path(&text("/etc/hosts"))), "ok /etc/hosts");
        assert_eq!(kind(path(&text("/"))), "ok /");
        assert!(matches!(path(&text("/a")).unwrap(), VfsPath::File(_)));
    }

    #[test]
    fn a_path_that_is_not_absolute_is_refused() {
        for bad in [
            "",
            "etc/hosts",
            "./etc",
            "../etc",
            "~/x",
            "file:///etc",
            "admin:///etc",
        ] {
            assert_eq!(kind(path(&text(bad))), "invalidLocation", "{bad:?}");
        }
    }

    #[test]
    fn dot_parts_and_untidy_separators_are_refused() {
        for bad in [
            "/a/../b",
            "/a/./b",
            "/..",
            "/a/..",
            "/./a",
            "/a//b",
            "/a/",
            "//a",
            "/a/b/../..",
        ] {
            assert_eq!(kind(path(&text(bad))), "invalidLocation", "{bad:?}");
        }
    }

    #[test]
    fn a_name_that_is_not_unicode_is_a_path_like_any_other() {
        let wire = WireOs::Bytes(vec![b'/', b'a', 0xff]);
        assert!(path(&wire).is_ok());
        assert_eq!(
            kind(path(&WireOs::Wide(vec![0x2f]))),
            "invalidLocation",
            "a form of another platform"
        );
    }

    #[test]
    fn a_nul_in_a_path_is_refused() {
        assert_eq!(kind(path(&text("/a\0b"))), "invalidLocation");
    }

    #[test]
    fn a_name_is_one_component() {
        for good in ["a", "a b", "..a", ".hidden", "日本語"] {
            assert!(name_ok(OsStr::new(good)), "{good:?}");
        }
        for bad in ["", ".", "..", "a/b", "/a", "a/", "a\0", "../x"] {
            assert!(!name_ok(OsStr::new(bad)), "{bad:?}");
        }
        assert!(matches!(
            name(&text("a/b")),
            Err(VfsError::InvalidName { .. })
        ));
        assert_eq!(name(&text("ok")).unwrap(), OsString::from("ok"));
    }

    #[test]
    fn a_table_that_is_full_refuses_one_more() {
        assert!(admit(63, 64, "handles").is_ok());
        assert!(matches!(admit(64, 64, "handles"), Err(VfsError::Io { .. })));
    }
}
