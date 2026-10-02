// Tests `.thumbnailer` parsing, `Exec` expansion and discovery in a temporary directory
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

const EVINCE: &str = "\
[Thumbnailer Entry]
TryExec=evince-thumbnailer
Exec=evince-thumbnailer -s %s %u %o
MimeType=application/pdf;application/x-bzpdf;application/x-gzpdf;
";

#[test]
fn a_thumbnailer_file_parses() {
    let t = Thumbnailer::parse("evince", EVINCE).unwrap();
    assert_eq!(t.id, "evince");
    assert_eq!(t.try_exec.as_deref(), Some("evince-thumbnailer"));
    assert_eq!(t.exec, "evince-thumbnailer -s %s %u %o");
    assert_eq!(
        t.mime_types,
        [
            "application/pdf",
            "application/x-bzpdf",
            "application/x-gzpdf"
        ]
    );
    assert!(t.handles("application/pdf"));
    assert!(!t.handles("image/png"));
}

#[test]
fn files_without_an_entry_exec_or_types_are_refused() {
    assert!(Thumbnailer::parse("a", "[Other]\nExec=x\nMimeType=a/b;\n").is_none());
    assert!(Thumbnailer::parse("a", "[Thumbnailer Entry]\nMimeType=a/b;\n").is_none());
    assert!(Thumbnailer::parse("a", "[Thumbnailer Entry]\nExec=x\n").is_none());
    assert!(Thumbnailer::parse("a", "").is_none());
    // Comments and keys of other groups are ignored.
    let t = Thumbnailer::parse(
        "a",
        "# c\n[Thumbnailer Entry]\n# Exec=no\nExec=x %i\nMimeType=a/b\n[Extra]\nExec=ignored\n",
    )
    .unwrap();
    assert_eq!(t.exec, "x %i");
    assert_eq!(t.mime_types, ["a/b"]);
}

#[test]
fn the_exec_line_expands_its_codes() {
    let t = Thumbnailer::parse("evince", EVINCE).unwrap();
    let argv = t
        .command(&ExecValues {
            input: Path::new("/home/a b/doc.pdf"),
            uri: "file:///home/a%20b/doc.pdf",
            output: Path::new("/tmp/out.png"),
            size: 256,
        })
        .unwrap();
    assert_eq!(
        argv,
        [
            "evince-thumbnailer",
            "-s",
            "256",
            "file:///home/a%20b/doc.pdf",
            "/tmp/out.png"
        ]
    );
}

#[test]
fn values_with_spaces_or_percent_signs_stay_one_argument_and_are_not_expanded_again() {
    let t = Thumbnailer::parse(
        "t",
        "[Thumbnailer Entry]\nExec=tool --in=%i --pct=100%% %o\nMimeType=a/b;\n",
    )
    .unwrap();
    let argv = t
        .command(&ExecValues {
            input: Path::new("/a b/%o %s.x"),
            uri: "u",
            output: Path::new("/o"),
            size: 1,
        })
        .unwrap();
    assert_eq!(argv, ["tool", "--in=/a b/%o %s.x", "--pct=100%", "/o"]);
}

#[test]
fn exec_quoting_follows_the_desktop_entry_rules() {
    assert_eq!(split_exec("a  b\tc"), ["a", "b", "c"]);
    assert_eq!(split_exec(r#"a "b c" d"#), ["a", "b c", "d"]);
    assert_eq!(
        split_exec(r#"a "b \"q\" \\ \$ \x""#),
        ["a", r#"b "q" \ $ \x"#]
    );
    assert_eq!(split_exec(r#"a "" b"#), ["a", "", "b"]);
    assert!(split_exec("   ").is_empty());
    let t = Thumbnailer {
        id: "t".into(),
        try_exec: None,
        exec: "".into(),
        mime_types: vec![],
    };
    let values = ExecValues {
        input: Path::new("/i"),
        uri: "u",
        output: Path::new("/o"),
        size: 1,
    };
    assert_eq!(t.command(&values), None);
}

#[cfg(unix)]
fn write_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    fs::write(path, "#!/bin/sh\n").unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(unix)]
#[test]
fn discovery_honours_try_exec_the_order_of_directories_and_hiding() {
    let tmp = tempfile::tempdir().unwrap();
    let (first, second, bin) = (
        tmp.path().join("one"),
        tmp.path().join("two"),
        tmp.path().join("bin"),
    );
    for dir in [&first, &second, &bin] {
        fs::create_dir_all(dir).unwrap();
    }
    fs::create_dir_all(first.join("thumbnailers")).unwrap();
    fs::create_dir_all(second.join("thumbnailers")).unwrap();
    write_executable(&bin.join("evince-thumbnailer"));
    fs::write(first.join("thumbnailers/evince.thumbnailer"), EVINCE).unwrap();
    // The same file name in a later directory is hidden.
    fs::write(
        second.join("thumbnailers/evince.thumbnailer"),
        "[Thumbnailer Entry]\nExec=other %i %o\nMimeType=text/plain;\n",
    )
    .unwrap();
    // A missing TryExec program drops the thumbnailer.
    fs::write(
        second.join("thumbnailers/gone.thumbnailer"),
        "[Thumbnailer Entry]\nTryExec=no-such-program\nExec=gone %i %o\nMimeType=a/b;\n",
    )
    .unwrap();
    fs::write(
        second.join("thumbnailers/b.thumbnailer"),
        "[Thumbnailer Entry]\nExec=b %i %o\nMimeType=a/b;\n",
    )
    .unwrap();
    fs::write(
        second.join("thumbnailers/a.thumbnailer"),
        "[Thumbnailer Entry]\nExec=a %i %o\nMimeType=a/b;\n",
    )
    .unwrap();
    fs::write(second.join("thumbnailers/notes.txt"), "ignored").unwrap();

    let found = discover(&[first, second, tmp.path().join("missing")], &[bin]);
    let ids: Vec<&str> = found.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, ["evince", "a", "b"]);
    assert_eq!(found[0].exec, "evince-thumbnailer -s %s %u %o");
}

#[cfg(unix)]
#[test]
fn try_exec_with_a_path_must_be_an_executable_file() {
    let tmp = tempfile::tempdir().unwrap();
    let exe = tmp.path().join("tool");
    write_executable(&exe);
    let plain = tmp.path().join("plain");
    fs::write(&plain, "x").unwrap();
    assert!(program_exists(exe.to_str().unwrap(), &[]));
    assert!(!program_exists(plain.to_str().unwrap(), &[]));
    assert!(!program_exists("/no/such/tool", &[]));
    assert!(program_exists("tool", &[tmp.path().to_path_buf()]));
}
