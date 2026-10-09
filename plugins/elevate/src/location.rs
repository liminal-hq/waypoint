// Compares Windows paths as text, without the file system: which folder a path is under, and whether two paths are the same
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// A path as its drive and folder names in lower case, or `None` when it is not a plain absolute drive path. Accepted: `C:\a\b` (either separator, repeated separators collapsed, `.` dropped) and the verbatim form of a drive path, `\\?\C:\a\b`. Refused, because they can name one thing in two ways or reach outside the folder they appear to be in: relative paths, `..`, network (UNC) and device paths, `\\?\` forms other than a drive, alternate data streams, names Windows trims (a trailing dot or space) and wildcard or reserved characters.
pub fn normalise(path: &str) -> Option<Vec<String>> {
    let path = path.strip_prefix(r"\\?\").unwrap_or(path);
    let mut parts = path.split(['\\', '/']);
    let drive = parts.next()?;
    let bytes = drive.as_bytes();
    if bytes.len() != 2 || !bytes[0].is_ascii_alphabetic() || bytes[1] != b':' {
        return None;
    }
    let mut components = vec![drive.to_ascii_lowercase()];
    for part in parts {
        match part {
            "" | "." => continue,
            ".." => return None,
            _ => {}
        }
        if part.ends_with(['.', ' '])
            || part
                .chars()
                .any(|c| c.is_control() || matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        {
            return None;
        }
        components.push(part.to_lowercase());
    }
    Some(components)
}

/// True when `path` is strictly inside `folder`, compared by whole components and ignoring case. `C:\Program Files Evil\x` is not inside `C:\Program Files`, and neither is the folder itself. Either path that is not plain (see [`normalise`]) makes the answer false.
pub fn is_under(path: &str, folder: &str) -> bool {
    match (normalise(path), normalise(folder)) {
        (Some(path), Some(folder)) => {
            folder.len() >= 2 && path.len() > folder.len() && path[..folder.len()] == folder[..]
        }
        _ => false,
    }
}

/// True when both are plain paths and name the same place by text.
pub fn same_path(a: &str, b: &str) -> bool {
    matches!((normalise(a), normalise(b)), (Some(a), Some(b)) if a == b)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PF: &str = r"C:\Program Files";

    #[test]
    fn a_path_inside_the_folder_is_under_it() {
        assert!(is_under(r"C:\Program Files\Tool\helper.exe", PF));
        assert!(is_under(r"c:\program files\TOOL\helper.exe", PF));
        assert!(is_under(r"C:/Program Files/Tool/helper.exe", PF));
        assert!(is_under(r"C:\Program Files\\Tool\.\helper.exe", PF));
        assert!(is_under(r"C:\Program Files\Tool", r"C:\Program Files\"));
    }

    #[test]
    fn the_folder_itself_a_sibling_and_a_lookalike_are_not_under_it() {
        assert!(!is_under(PF, PF));
        assert!(!is_under(r"C:\Program Files\", PF));
        assert!(!is_under(r"C:\Program Files Evil\helper.exe", PF));
        assert!(!is_under(r"C:\Program Files (x86)\helper.exe", PF));
        assert!(!is_under(r"D:\Program Files\helper.exe", PF));
        assert!(!is_under(r"C:\Users\me\Program Files\helper.exe", PF));
        assert!(!is_under(r"C:\helper.exe", PF));
    }

    #[test]
    fn parent_references_and_odd_forms_are_refused() {
        assert!(!is_under(r"C:\Program Files\..\Users\me\helper.exe", PF));
        assert!(!is_under(r"C:\Users\me\..\..\Program Files\x.exe", PF));
        assert!(!is_under(r"\\?\UNC\server\share\Program Files\x.exe", PF));
        assert!(!is_under(r"\\server\share\Program Files\x.exe", PF));
        assert!(!is_under(r"\\.\C:\Program Files\x.exe", PF));
        assert!(!is_under(
            r"\\?\GLOBALROOT\Device\x\Program Files\x.exe",
            PF
        ));
        assert!(!is_under(r"Program Files\x.exe", PF));
        assert!(!is_under(r"\Program Files\x.exe", PF));
        assert!(!is_under(r"C:\Program Files\x.exe:stream", PF));
        assert!(!is_under(r"C:\Program Files\Tool.\x.exe", PF));
        assert!(!is_under(r"C:\Program Files\Tool \x.exe", PF));
        assert!(!is_under(r"C:\Program Files\*\x.exe", PF));
        assert!(!is_under("", PF));
        assert!(!is_under(r"C:\Program Files\x.exe", ""));
        assert!(!is_under(r"C:\Program Files\x.exe", "C:"));
    }

    #[test]
    fn the_verbatim_drive_form_is_the_same_place() {
        assert!(is_under(r"\\?\C:\Program Files\Tool\h.exe", PF));
        assert!(same_path(
            r"\\?\C:\Program Files\Tool\h.exe",
            r"c:\program files\tool\H.EXE"
        ));
    }

    #[test]
    fn same_path_ignores_case_and_separators_only() {
        assert!(same_path(r"C:\A\b.exe", r"c:/a//B.EXE"));
        assert!(!same_path(r"C:\A\b.exe", r"C:\A\c.exe"));
        assert!(!same_path(r"C:\A\b.exe", r"C:\A"));
        assert!(!same_path(r"C:\A\..\b.exe", r"C:\b.exe"));
        assert!(!same_path("", ""));
    }
}
