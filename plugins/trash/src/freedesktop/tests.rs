// Tests the freedesktop trash in temporary directories with fake mounts, devices and clocks
//
// Nothing here touches the real `~/.local/share/Trash`: the environment is injected, and the "volumes" are folders of a temporary directory whose device numbers a fake `TrashFs` makes up.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::{symlink, DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use chrono::{Duration, NaiveDate, NaiveDateTime};
use tempfile::TempDir;

use super::*;

const UID: u32 = 1000;

fn start() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 10, 1)
        .unwrap()
        .and_hms_opt(12, 0, 0)
        .unwrap()
}

/// A file system whose devices are folders: everything is device 1 except what is under a registered folder. A rename between two devices fails with `EXDEV`, as a real one does, and a failure can be forced.
struct FakeFs {
    devices: Vec<(PathBuf, u64)>,
    fail_rename: Mutex<Option<i32>>,
    /// Owners to report instead of the real one, by path: how another user's folder is faked.
    owners: Mutex<BTreeMap<PathBuf, u32>>,
    /// Every path `device_id` was asked about.
    device_asked: Mutex<Vec<PathBuf>>,
}

impl FakeFs {
    fn device_of(&self, path: &Path) -> u64 {
        self.devices
            .iter()
            .filter(|(root, _)| path.starts_with(root))
            .max_by_key(|(root, _)| root.components().count())
            .map_or(1, |(_, device)| *device)
    }

    fn check_rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        if let Some(code) = *self.fail_rename.lock().unwrap() {
            return Err(io::Error::from_raw_os_error(code));
        }
        if self.device_of(from) != self.device_of(to) {
            return Err(io::Error::from_raw_os_error(libc::EXDEV));
        }
        Ok(())
    }
}

impl TrashFs for FakeFs {
    /// Everything the test creates belongs to `UID`, whoever runs the tests, unless a test names another owner.
    fn owner_and_mode(&self, path: &Path) -> io::Result<(u32, u32)> {
        let metadata = std::fs::symlink_metadata(path)?;
        let owner = self.owners.lock().unwrap().get(path).copied();
        Ok((owner.unwrap_or(UID), metadata.mode()))
    }

    fn device_id(&self, path: &Path) -> io::Result<u64> {
        self.device_asked.lock().unwrap().push(path.to_path_buf());
        std::fs::symlink_metadata(path)?;
        Ok(self.device_of(path))
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.check_rename(from, to)?;
        StdFs.rename(from, to)
    }

    fn rename_noreplace(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.check_rename(from, to)?;
        StdFs.rename_noreplace(from, to)
    }
}

struct Fx {
    _tmp: TempDir,
    /// The temporary directory, where ordinary files live (device 1, the home device).
    root: PathBuf,
    home: PathBuf,
    data_home: PathBuf,
    /// A volume mounted at `root/mnt/usb` (device 2).
    usb: PathBuf,
    /// A second volume at `root/mnt/disk` (device 3).
    disk: PathBuf,
    fs: Arc<FakeFs>,
    clock: Arc<Mutex<NaiveDateTime>>,
    env: TrashEnv,
}

impl Fx {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(tmp.path()).unwrap();
        let home = root.join("home");
        let data_home = home.join(".local/share");
        let usb = root.join("mnt/usb");
        let disk = root.join("mnt/disk");
        for dir in [&home, &usb, &disk, &root.join("work")] {
            std::fs::create_dir_all(dir).unwrap();
        }
        let clock = Arc::new(Mutex::new(start()));
        let now = Arc::clone(&clock);
        let env = TrashEnv {
            data_home: data_home.clone(),
            home_dir: home.clone(),
            uid: UID,
            mounts: vec![
                MountInfo {
                    mount_point: root.clone(),
                    device_id: 1,
                    is_network: false,
                    is_removable: false,
                },
                MountInfo {
                    mount_point: usb.clone(),
                    device_id: 2,
                    is_network: false,
                    is_removable: true,
                },
                MountInfo {
                    mount_point: disk.clone(),
                    device_id: 3,
                    is_network: true,
                    is_removable: false,
                },
            ],
            now: Arc::new(move || *now.lock().unwrap()),
        };
        let fs = Arc::new(FakeFs {
            devices: vec![(usb.clone(), 2), (disk.clone(), 3)],
            fail_rename: Mutex::new(None),
            owners: Mutex::new(BTreeMap::new()),
            device_asked: Mutex::new(Vec::new()),
        });
        Fx {
            _tmp: tmp,
            root,
            home,
            data_home,
            usb,
            disk,
            fs,
            clock,
            env,
        }
    }

    fn trash(&self) -> Freedesktop {
        Freedesktop::with_fs(self.env.clone(), self.fs.clone())
    }

    fn work(&self, relative: &str) -> PathBuf {
        self.root.join("work").join(relative)
    }

    fn home_trash(&self) -> PathBuf {
        self.data_home.join("Trash")
    }

    fn advance(&self, duration: Duration) {
        *self.clock.lock().unwrap() += duration;
    }

    fn fail_renames_with(&self, code: Option<i32>) {
        *self.fs.fail_rename.lock().unwrap() = code;
    }
}

fn write(path: &Path, content: &str) -> PathBuf {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
    path.to_path_buf()
}

/// Makes `path` and any parents missing with mode `700`, the mode a trash folder needs, whatever the
/// umask of whoever runs the tests (`create_dir_all` takes `775` under `002`, which the plugin refuses).
fn private_dirs(path: &Path) {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
        .unwrap();
}

fn names(dir: &Path) -> Vec<String> {
    let mut found: Vec<String> = std::fs::read_dir(dir)
        .map(|read| {
            read.flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found
}

fn mode(path: &Path) -> u32 {
    std::fs::symlink_metadata(path)
        .unwrap()
        .permissions()
        .mode()
        & 0o7777
}

fn is_root_user() -> bool {
    // SAFETY: `geteuid` has no arguments and cannot fail.
    unsafe { libc::geteuid() == 0 }
}

fn bytes_name(bytes: &[u8]) -> OsString {
    OsString::from_vec(bytes.to_vec())
}

// ------------------------------------------------------------------ home trash

#[test]
fn a_file_goes_to_the_home_trash_and_comes_back() {
    let fx = Fx::new();
    let file = write(&fx.work("report.txt"), "quarterly");
    let receipt = fx.trash().trash(&file).unwrap();

    assert!(!file.exists());
    assert_eq!(receipt.original_path, file);
    assert_eq!(receipt.deleted_at, to_unix(&start()));
    let trash = fx.home_trash();
    assert_eq!(
        std::fs::read_to_string(trash.join("files/report.txt")).unwrap(),
        "quarterly"
    );
    assert_eq!(
        std::fs::read_to_string(trash.join("info/report.txt.trashinfo")).unwrap(),
        format!(
            "[Trash Info]\nPath={}\nDeletionDate=2026-10-01T12:00:00\n",
            encode(file.as_os_str().as_encoded_bytes())
        )
    );
    // The spec asks for 0700 on the directories a trash creates.
    assert_eq!(mode(&trash), 0o700);
    assert_eq!(mode(&trash.join("files")), 0o700);
    assert_eq!(mode(&trash.join("info")), 0o700);

    let restored = fx
        .trash()
        .restore(&receipt.trash_id, &RestoreTarget::Original)
        .unwrap();
    assert_eq!(restored.original_path, file);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "quarterly");
    assert!(names(&trash.join("files")).is_empty());
    assert!(names(&trash.join("info")).is_empty());
}

#[test]
fn the_home_trash_is_created_only_when_something_goes_in_it() {
    let fx = Fx::new();
    assert!(fx.trash().list().is_empty());
    assert!(!fx.home_trash().exists());
    write(&fx.work("a"), "x");
    fx.trash().trash(&fx.work("a")).unwrap();
    assert!(fx.home_trash().exists());
}

#[test]
fn a_folder_is_trashed_whole_and_restored_whole() {
    let fx = Fx::new();
    write(&fx.work("project/src/main.rs"), "fn main() {}");
    write(&fx.work("project/README"), "hi");
    let receipt = fx.trash().trash(&fx.work("project")).unwrap();
    assert!(!fx.work("project").exists());
    assert!(fx.home_trash().join("files/project/src/main.rs").exists());
    fx.trash()
        .restore(&receipt.trash_id, &RestoreTarget::Original)
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(fx.work("project/src/main.rs")).unwrap(),
        "fn main() {}"
    );
}

#[test]
fn colliding_names_get_a_number_before_the_extension() {
    let fx = Fx::new();
    let mut seen = Vec::new();
    for folder in ["a", "b", "c"] {
        for name in ["photo.jpg", "notes", ".bashrc", "archive.tar.gz"] {
            let path = write(&fx.work(&format!("{folder}/{name}")), folder);
            fx.trash().trash(&path).unwrap();
            seen.push(name);
        }
    }
    assert_eq!(
        names(&fx.home_trash().join("files")),
        [
            ".bashrc",
            ".bashrc.2",
            ".bashrc.3",
            "archive.tar.2.gz",
            "archive.tar.3.gz",
            "archive.tar.gz",
            "notes",
            "notes.2",
            "notes.3",
            "photo.2.jpg",
            "photo.3.jpg",
            "photo.jpg",
        ]
    );
    // Each `.trashinfo` still points at the right place.
    let items = fx.trash().list();
    assert_eq!(items.len(), 12);
    for item in &items {
        let content = std::fs::read_to_string(
            fx.home_trash().join("files").join(
                item.receipt
                    .trash_id
                    .split('|')
                    .nth(1)
                    .unwrap()
                    .replace("%2E", "."),
            ),
        );
        let folder = item
            .original_path
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert_eq!(content.unwrap(), folder, "{item:?}");
    }
}

#[test]
fn an_item_without_a_trashinfo_still_holds_its_name() {
    let fx = Fx::new();
    let trash = fx.home_trash();
    private_dirs(&trash.join("files"));
    private_dirs(&trash.join("info"));
    write(&trash.join("files/a.txt"), "someone else's");
    let file = write(&fx.work("a.txt"), "mine");
    fx.trash().trash(&file).unwrap();
    assert_eq!(
        std::fs::read_to_string(trash.join("files/a.txt")).unwrap(),
        "someone else's"
    );
    assert_eq!(
        std::fs::read_to_string(trash.join("files/a.2.txt")).unwrap(),
        "mine"
    );
    assert!(!trash.join("info/a.txt.trashinfo").exists());
}

#[test]
fn a_name_that_is_too_long_for_its_trashinfo_is_shortened() {
    let fx = Fx::new();
    for stem in ["a".repeat(250), "é".repeat(125)] {
        let file = write(&fx.work(&format!("{stem}.txt")), "long");
        // The name fits the file system, but `name.trashinfo` would not.
        let receipt = fx.trash().trash(&file).unwrap();
        for entry in std::fs::read_dir(fx.home_trash().join("info"))
            .unwrap()
            .flatten()
        {
            assert!(entry.file_name().len() <= 255);
        }
        fx.trash()
            .restore(&receipt.trash_id, &RestoreTarget::Original)
            .unwrap();
        assert!(file.exists());
    }
}

#[test]
fn candidate_names_never_cut_a_character() {
    let name = bytes_name("é".repeat(130).as_bytes());
    let candidate = candidate_name(&name, 0);
    assert!(candidate.len() + INFO_SUFFIX.len() <= NAME_MAX);
    assert!(candidate.to_str().is_some(), "{candidate:?}");
    let numbered = candidate_name(&bytes_name(b"x.txt"), 4);
    assert_eq!(numbered, OsString::from("x.5.txt"));
}

const INFO_SUFFIX: &str = ".trashinfo";

// -------------------------------------------------------------- trashinfo text

#[test]
fn awkward_names_survive_the_trashinfo() {
    let fx = Fx::new();
    let awkward: Vec<OsString> = vec![
        "with space.txt".into(),
        "100%.txt".into(),
        "#hash.txt".into(),
        "line\nbreak.txt".into(),
        "tab\there".into(),
        "ünïcode — dash".into(),
        "[Trash Info]".into(),
        "Path=sneaky".into(),
        bytes_name(b"raw\xff\xfe.bin"),
        "%41".into(),
    ];
    for name in &awkward {
        let file = fx.work("odd").join(name);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, name.as_encoded_bytes()).unwrap();
        let receipt = fx.trash().trash(&file).unwrap();
        // The `.trashinfo` is two clean lines however odd the name.
        let info = std::fs::read(
            fx.home_trash()
                .join("info")
                .join(info_file_name(&decode_os_from_id(&receipt.trash_id))),
        )
        .unwrap();
        let text = String::from_utf8(info).unwrap();
        assert_eq!(text.lines().count(), 3, "{text:?}");
        assert!(text.is_ascii());
        // It restores to the identical raw path.
        let restored = fx
            .trash()
            .restore(&receipt.trash_id, &RestoreTarget::Original)
            .unwrap();
        assert_eq!(restored.original_path, file);
        assert_eq!(std::fs::read(&file).unwrap(), name.as_encoded_bytes());
    }
}

fn decode_os_from_id(id: &str) -> OsString {
    parse_id(id).unwrap().1
}

#[test]
fn a_folder_that_is_not_utf8_round_trips() {
    let fx = Fx::new();
    let dir = fx.work("").join(bytes_name(b"d\xe9j\xe0"));
    let file = write(&dir.join("f"), "x");
    let receipt = fx.trash().trash(&file).unwrap();
    let items = fx.trash().list();
    assert_eq!(
        items[0].original_path, file,
        "the raw path survives listing"
    );
    fx.trash()
        .restore(&receipt.trash_id, &RestoreTarget::Original)
        .unwrap();
    assert!(file.exists());
}

#[test]
fn percent_encoding_round_trips_every_byte() {
    let all: Vec<u8> = (0..=255).collect();
    let encoded = encode(&all);
    assert!(encoded.is_ascii());
    assert!(!encoded.contains('\n') && !encoded.contains(' ') && !encoded.contains('#'));
    assert_eq!(decode(&encoded), all);
    // Lowercase hex and sloppy writers are read, a stray `%` is kept.
    assert_eq!(decode("a%2fb%2Fc"), b"a/b/c");
    assert_eq!(decode("100%"), b"100%");
    assert_eq!(decode("%zz%4"), b"%zz%4");
}

#[test]
fn trashinfo_files_from_other_tools_are_read_leniently() {
    use trashinfo::{parse_date, parse_info};
    let date = NaiveDate::from_ymd_opt(2024, 2, 29)
        .unwrap()
        .and_hms_opt(23, 59, 1)
        .unwrap();
    let crlf =
        parse_info("[Trash Info]\r\nPath=/home/u/a%20b\r\nDeletionDate=2024-02-29T23:59:01\r\n")
            .unwrap();
    assert_eq!(crlf.path, OsString::from("/home/u/a b"));
    assert_eq!(crlf.deleted_at, Some(date));
    // Another group first, a comment, repeated keys, spaces around the value.
    let messy = parse_info("[Other]\nPath=/wrong\n\n[Trash Info]\n# comment\nPath=/right\nPath=/wrong2\nDeletionDate=2024-02-29T23:59:01Z\nDeletionDate=1999-01-01T00:00:00\n").unwrap();
    assert_eq!(messy.path, OsString::from("/right"));
    assert_eq!(messy.deleted_at, Some(date));
    assert_eq!(
        parse_info("[Trash Info]\nPath=/a\nDeletionDate=garbage\n")
            .unwrap()
            .deleted_at,
        None
    );
    assert!(parse_info("[Trash Info]\nDeletionDate=2024-02-29T23:59:01\n").is_none());
    assert!(parse_info("Path=/a\n").is_none());
    assert!(parse_info("[Trash Info]\nPath=\n").is_none());
    assert_eq!(parse_date("2024-02-29T23:59:01.5"), Some(date));
}

#[test]
fn an_item_with_no_usable_date_is_not_given_a_date_that_would_expire_it() {
    let fx = Fx::new();
    let file = write(&fx.work("undated"), "x");
    let receipt = fx.trash().trash(&file).unwrap();
    let (_, name) = parse_id(&receipt.trash_id).unwrap();
    let info = fx.home_trash().join("info").join(info_file_name(&name));
    std::fs::write(
        &info,
        format!(
            "[Trash Info]\nPath={}\n",
            encode(file.as_os_str().as_encoded_bytes())
        ),
    )
    .unwrap();
    // Its date is the `.trashinfo` file's own time, which is now, so a sweep keeps it.
    fx.advance(Duration::days(1));
    let report = fx.trash().empty(Some(30));
    assert_eq!(report.removed, 0);
    assert_eq!(fx.trash().list().len(), 1);
}

// ------------------------------------------------------------ per-volume trash

#[test]
fn a_file_on_another_volume_goes_to_that_volumes_own_trash() {
    let fx = Fx::new();
    let file = write(&fx.usb.join("photos/cat.jpg"), "meow");
    let receipt = fx.trash().trash(&file).unwrap();

    let trash = fx.usb.join(format!(".Trash-{UID}"));
    assert_eq!(names(&trash.join("files")), ["cat.jpg"]);
    assert_eq!(mode(&trash), 0o700);
    assert_eq!(mode(&trash.join("files")), 0o700);
    // The path is relative to the volume's top directory.
    assert_eq!(
        std::fs::read_to_string(trash.join("info/cat.jpg.trashinfo")).unwrap(),
        "[Trash Info]\nPath=photos/cat.jpg\nDeletionDate=2026-10-01T12:00:00\n"
    );
    assert!(!fx.home_trash().join("files/cat.jpg").exists());
    assert_eq!(receipt.original_path, file);

    let items = fx.trash().list();
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].original_path, file,
        "a relative Path is joined to the volume"
    );
    fx.trash()
        .restore(&receipt.trash_id, &RestoreTarget::Original)
        .unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "meow");
}

#[test]
fn a_shared_trash_folder_with_the_sticky_bit_is_used_for_the_user() {
    let fx = Fx::new();
    let shared = fx.usb.join(".Trash");
    std::fs::create_dir(&shared).unwrap();
    std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o1777)).unwrap();
    let file = write(&fx.usb.join("a.txt"), "x");
    fx.trash().trash(&file).unwrap();
    let own = shared.join(UID.to_string());
    assert_eq!(names(&own.join("files")), ["a.txt"]);
    assert_eq!(mode(&own), 0o700);
    assert!(!fx.usb.join(format!(".Trash-{UID}")).exists());
    // The spec writes relative paths in this one too.
    let info = std::fs::read_to_string(own.join("info/a.txt.trashinfo")).unwrap();
    assert!(info.contains("Path=a.txt\n"), "{info}");
    assert_eq!(fx.trash().list().len(), 1);
}

#[test]
fn a_shared_trash_folder_without_the_sticky_bit_is_not_used() {
    let fx = Fx::new();
    let shared = fx.usb.join(".Trash");
    std::fs::create_dir(&shared).unwrap();
    std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o777)).unwrap();
    let file = write(&fx.usb.join("a.txt"), "x");
    fx.trash().trash(&file).unwrap();
    assert!(!shared.join(UID.to_string()).exists());
    assert_eq!(
        names(&fx.usb.join(format!(".Trash-{UID}/files"))),
        ["a.txt"]
    );
}

#[test]
fn a_shared_trash_folder_that_is_a_link_is_not_used() {
    let fx = Fx::new();
    let elsewhere = fx.root.join("elsewhere");
    std::fs::create_dir(&elsewhere).unwrap();
    std::fs::set_permissions(&elsewhere, std::fs::Permissions::from_mode(0o1777)).unwrap();
    symlink(&elsewhere, fx.usb.join(".Trash")).unwrap();
    let file = write(&fx.usb.join("a.txt"), "x");
    fx.trash().trash(&file).unwrap();
    assert!(names(&elsewhere).is_empty(), "the link was not followed");
    assert_eq!(
        names(&fx.usb.join(format!(".Trash-{UID}/files"))),
        ["a.txt"]
    );
}

#[test]
fn a_shared_trash_folder_that_is_a_file_falls_back_to_the_per_user_trash() {
    let fx = Fx::new();
    write(&fx.usb.join(".Trash"), "not a folder");
    let file = write(&fx.usb.join("a.txt"), "x");
    fx.trash().trash(&file).unwrap();
    assert_eq!(
        names(&fx.usb.join(format!(".Trash-{UID}/files"))),
        ["a.txt"]
    );
}

#[test]
fn a_shared_trash_user_folder_that_cannot_be_made_falls_back() {
    let fx = Fx::new();
    let shared = fx.usb.join(".Trash");
    std::fs::create_dir(&shared).unwrap();
    std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o1777)).unwrap();
    // A file where the user's folder should be.
    write(&shared.join(UID.to_string()), "in the way");
    let file = write(&fx.usb.join("a.txt"), "x");
    fx.trash().trash(&file).unwrap();
    assert_eq!(
        names(&fx.usb.join(format!(".Trash-{UID}/files"))),
        ["a.txt"]
    );
}

// ------------------------------------------------------- trust in a trash folder

impl Fx {
    /// Makes `path` look as if `owner` owns it.
    fn owned_by(&self, path: &Path, owner: u32) {
        self.fs
            .owners
            .lock()
            .unwrap()
            .insert(path.to_path_buf(), owner);
    }

    /// A per-user trash on the usb volume, laid out as `ensure_trash_dir` would, holding one item.
    fn planted_trash(&self, trash: &Path) {
        for dir in ["files", "info"] {
            std::fs::create_dir_all(trash.join(dir)).unwrap();
        }
        for dir in [trash.to_path_buf(), trash.join("files"), trash.join("info")] {
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        write(&trash.join("files/theirs.txt"), "theirs");
        write(
            &trash.join("info/theirs.txt.trashinfo"),
            "[Trash Info]\nPath=theirs.txt\nDeletionDate=2026-09-01T00:00:00\n",
        );
    }
}

#[test]
fn a_per_user_trash_that_another_user_owns_is_refused() {
    let fx = Fx::new();
    let planted = fx.usb.join(format!(".Trash-{UID}"));
    fx.planted_trash(&planted);
    fx.owned_by(&planted, UID + 1);
    let file = write(&fx.usb.join("a.txt"), "x");
    let error = fx.trash().trash(&file).unwrap_err();
    assert!(
        matches!(error, TrashError::TrashUnavailable { .. }),
        "{error:?}"
    );
    assert!(file.exists(), "the file stays where it was");
    assert_eq!(names(&planted.join("files")), ["theirs.txt"]);
}

#[test]
fn a_shared_user_folder_that_another_user_owns_falls_back_to_the_per_user_trash() {
    let fx = Fx::new();
    let shared = fx.usb.join(".Trash");
    std::fs::create_dir(&shared).unwrap();
    std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o1777)).unwrap();
    let planted = shared.join(UID.to_string());
    fx.planted_trash(&planted);
    fx.owned_by(&planted, UID + 1);
    let file = write(&fx.usb.join("a.txt"), "x");
    fx.trash().trash(&file).unwrap();
    assert_eq!(names(&planted.join("files")), ["theirs.txt"]);
    assert_eq!(
        names(&fx.usb.join(format!(".Trash-{UID}/files"))),
        ["a.txt"]
    );
}

#[test]
fn a_trash_folder_that_others_can_write_is_refused() {
    for (relative, unsafe_mode) in [("", 0o770), ("", 0o707), ("files", 0o775), ("info", 0o757)] {
        let fx = Fx::new();
        let planted = fx.usb.join(format!(".Trash-{UID}"));
        fx.planted_trash(&planted);
        let weak = planted.join(relative);
        std::fs::set_permissions(&weak, std::fs::Permissions::from_mode(unsafe_mode)).unwrap();
        let file = write(&fx.usb.join("a.txt"), "x");
        let error = fx.trash().trash(&file).unwrap_err();
        assert!(
            matches!(error, TrashError::TrashUnavailable { .. }),
            "{relative} {unsafe_mode:o}: {error:?}"
        );
        assert!(file.exists());
    }
}

#[test]
fn a_per_user_trash_that_is_a_link_is_refused() {
    let fx = Fx::new();
    let elsewhere = fx.root.join("elsewhere");
    fx.planted_trash(&elsewhere);
    symlink(&elsewhere, fx.usb.join(format!(".Trash-{UID}"))).unwrap();
    let file = write(&fx.usb.join("a.txt"), "x");
    let error = fx.trash().trash(&file).unwrap_err();
    assert!(
        matches!(error, TrashError::TrashUnavailable { .. }),
        "{error:?}"
    );
    assert_eq!(names(&elsewhere.join("files")), ["theirs.txt"]);
}

#[test]
fn an_untrusted_trash_is_neither_listed_nor_emptied() {
    let fx = Fx::new();
    let planted = fx.usb.join(format!(".Trash-{UID}"));
    fx.planted_trash(&planted);
    fx.owned_by(&planted, UID + 1);
    assert!(fx.trash().list().is_empty());
    let report = fx.trash().empty(None);
    assert_eq!(report, EmptyReport::default());
    assert_eq!(names(&planted.join("files")), ["theirs.txt"]);
    // The same trash, once it is ours, is listed.
    fx.fs.owners.lock().unwrap().clear();
    assert_eq!(fx.trash().list().len(), 1);
}

#[test]
fn removing_a_tree_unlinks_links_and_never_follows_them() {
    let fx = Fx::new();
    let outside = fx.root.join("outside");
    write(&outside.join("precious"), "keep");
    let tree = fx.work("tree");
    write(&tree.join("a/b/file"), "x");
    symlink(&outside, tree.join("a/link")).unwrap();
    symlink(outside.join("precious"), tree.join("filelink")).unwrap();
    remove_tree(&tree).unwrap();
    assert!(!tree.exists());
    assert_eq!(names(&outside), ["precious"]);
}

#[test]
fn a_folder_swapped_for_a_link_is_not_opened_through_it() {
    let fx = Fx::new();
    let outside = fx.root.join("outside");
    write(&outside.join("precious"), "keep");
    symlink(&outside, fx.work("swapped")).unwrap();
    let parent = open_dir(
        libc::AT_FDCWD,
        &c_string(fx.work("").as_os_str()).unwrap(),
        true,
    )
    .unwrap();
    // The descriptor-based open refuses a link in the last part...
    let error = open_dir(
        parent.as_raw_fd(),
        &c_string(OsStr::new("swapped")).unwrap(),
        false,
    )
    .unwrap_err();
    assert!(
        matches!(error.raw_os_error(), Some(libc::ELOOP | libc::ENOTDIR)),
        "{error:?}"
    );
    // ...and removing the entry removes the link, not the folder it points at.
    remove_entry(
        parent.as_raw_fd(),
        &c_string(OsStr::new("swapped")).unwrap(),
    )
    .unwrap();
    assert!(!fx.work("swapped").exists());
    assert_eq!(names(&outside), ["precious"]);
}

#[test]
fn a_tree_with_unreadable_and_read_only_folders_is_removed() {
    if is_root_user() {
        return;
    }
    let fx = Fx::new();
    let tree = fx.work("locked");
    write(&tree.join("closed/inner/file"), "x");
    write(&tree.join("readonly/file"), "x");
    std::fs::set_permissions(
        tree.join("closed/inner"),
        std::fs::Permissions::from_mode(0o000),
    )
    .unwrap();
    std::fs::set_permissions(tree.join("closed"), std::fs::Permissions::from_mode(0o300)).unwrap();
    std::fs::set_permissions(
        tree.join("readonly"),
        std::fs::Permissions::from_mode(0o500),
    )
    .unwrap();
    remove_tree(&tree).unwrap();
    assert!(!tree.exists());
}

#[test]
fn a_network_mount_is_not_asked_for_its_device_until_a_file_under_it_is_trashed() {
    let fx = Fx::new();
    // The environment leaves a network mount's device unknown (0), as `default_env` does.
    let mut env = fx.env.clone();
    for mount in &mut env.mounts {
        if mount.is_network {
            mount.device_id = 0;
        }
    }
    let trash = Freedesktop::with_fs(env, fx.fs.clone());
    let asked = |fx: &Fx| {
        fx.fs
            .device_asked
            .lock()
            .unwrap()
            .iter()
            .any(|path| path.starts_with(&fx.disk))
    };

    trash.trash(&write(&fx.work("a"), "x")).unwrap();
    assert!(trash.list().len() == 1);
    assert!(!asked(&fx), "an unrelated trash must not touch the mount");

    trash.trash(&write(&fx.disk.join("d"), "x")).unwrap();
    assert!(asked(&fx));
    assert_eq!(names(&fx.disk.join(format!(".Trash-{UID}/files"))), ["d"]);
}

#[test]
fn a_volume_without_a_usable_trash_is_an_error_and_nothing_moves() {
    let fx = Fx::new();
    // A file named like the trash, so it cannot be created.
    write(&fx.usb.join(format!(".Trash-{UID}")), "in the way");
    let file = write(&fx.usb.join("a.txt"), "x");
    let error = fx.trash().trash(&file).unwrap_err();
    assert!(
        matches!(error, TrashError::TrashUnavailable { .. }),
        "{error:?}"
    );
    assert!(file.exists(), "the file stays where it was");
    // Not copied into the home trash either.
    assert!(!fx.home_trash().join("files/a.txt").exists());
}

#[test]
fn a_file_on_a_volume_with_no_known_mount_is_unavailable() {
    let fx = Fx::new();
    let mut env = fx.env.clone();
    env.mounts.retain(|mount| mount.device_id != 2);
    let file = write(&fx.usb.join("a.txt"), "x");
    let error = Freedesktop::with_fs(env, fx.fs.clone())
        .trash(&file)
        .unwrap_err();
    assert!(
        matches!(error, TrashError::TrashUnavailable { .. }),
        "{error:?}"
    );
    assert!(file.exists());
}

#[test]
fn the_deepest_mount_of_the_device_holds_the_trash() {
    let fx = Fx::new();
    // A volume mounted inside another one of the same device (a bind mount): the nearer top directory wins.
    let inner = fx.usb.join("inner");
    std::fs::create_dir(&inner).unwrap();
    let mut env = fx.env.clone();
    env.mounts.push(MountInfo {
        mount_point: inner.clone(),
        device_id: 2,
        is_network: false,
        is_removable: false,
    });
    let file = write(&inner.join("deep/a.txt"), "x");
    Freedesktop::with_fs(env, fx.fs.clone())
        .trash(&file)
        .unwrap();
    assert_eq!(names(&inner.join(format!(".Trash-{UID}/files"))), ["a.txt"]);
    assert!(!fx.usb.join(format!(".Trash-{UID}")).exists());
}

#[test]
fn a_file_on_the_home_device_under_another_mount_point_uses_the_home_trash() {
    let fx = Fx::new();
    // The root volume (device 1, the home device) is a mount too; its files are not trashed on a `.Trash-uid` there.
    let file = write(&fx.work("a.txt"), "x");
    fx.trash().trash(&file).unwrap();
    assert!(!fx.root.join(format!(".Trash-{UID}")).exists());
    assert_eq!(names(&fx.home_trash().join("files")), ["a.txt"]);
}

#[test]
fn the_list_covers_the_home_trash_and_every_volume() {
    let fx = Fx::new();
    let a = write(&fx.work("home-file"), "1");
    let b = write(&fx.usb.join("usb-file"), "2");
    let c = write(&fx.disk.join("dir/disk-file"), "3");
    let shared = fx.disk.join(".Trash");
    std::fs::create_dir(&shared).unwrap();
    std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o1777)).unwrap();
    fx.trash().trash(&a).unwrap();
    fx.advance(Duration::minutes(1));
    fx.trash().trash(&b).unwrap();
    fx.advance(Duration::minutes(1));
    fx.trash().trash(&c).unwrap();

    let items = fx.trash().list();
    let paths: Vec<&PathBuf> = items.iter().map(|item| &item.original_path).collect();
    assert_eq!(paths, [&a, &b, &c], "oldest first");
    assert_eq!(items[0].name, "home-file");
    assert_eq!(items[1].size, 1);
    assert!(items.iter().all(|item| !item.is_dir));
    // Each item restores from its own trash.
    for item in &items {
        fx.trash()
            .restore(&item.receipt.trash_id, &RestoreTarget::Original)
            .unwrap();
    }
    assert!(a.exists() && b.exists() && c.exists());
    assert!(fx.trash().list().is_empty());
}

#[test]
fn a_rename_across_devices_is_an_error_and_leaves_no_trashinfo() {
    let fx = Fx::new();
    let file = write(&fx.usb.join("a.txt"), "x");
    fx.fail_renames_with(Some(libc::EXDEV));
    let error = fx.trash().trash(&file).unwrap_err();
    assert!(
        matches!(error, TrashError::TrashUnavailable { .. }),
        "{error:?}"
    );
    fx.fail_renames_with(None);
    assert!(file.exists());
    let trash = fx.usb.join(format!(".Trash-{UID}"));
    assert!(names(&trash.join("info")).is_empty(), "rolled back");
    assert!(names(&trash.join("files")).is_empty());
}

#[test]
fn the_fake_file_system_reports_a_real_cross_device_rename() {
    let fx = Fx::new();
    let file = write(&fx.usb.join("a.txt"), "x");
    let error = fx.fs.rename(&file, &fx.work("b.txt")).unwrap_err();
    assert_eq!(error.raw_os_error(), Some(libc::EXDEV));
}

#[test]
fn a_failed_rename_rolls_the_trashinfo_back() {
    let fx = Fx::new();
    let file = write(&fx.work("a.txt"), "x");
    for (code, expected) in [
        (libc::EACCES, TrashError::PermissionDenied),
        (libc::EPERM, TrashError::PermissionDenied),
        (libc::ENOENT, TrashError::NotFound),
    ] {
        fx.fail_renames_with(Some(code));
        assert_eq!(fx.trash().trash(&file).unwrap_err(), expected);
        assert!(
            names(&fx.home_trash().join("info")).is_empty(),
            "errno {code}"
        );
        assert!(file.exists());
    }
    fx.fail_renames_with(Some(libc::EIO));
    assert!(matches!(
        fx.trash().trash(&file).unwrap_err(),
        TrashError::Io { .. }
    ));
    assert!(names(&fx.home_trash().join("info")).is_empty());
    fx.fail_renames_with(None);
    fx.trash().trash(&file).unwrap();
}

#[test]
fn a_file_in_a_folder_that_cannot_be_written_is_permission_denied() {
    if is_root_user() {
        return;
    }
    let fx = Fx::new();
    let file = write(&fx.work("locked/a.txt"), "x");
    std::fs::set_permissions(fx.work("locked"), std::fs::Permissions::from_mode(0o500)).unwrap();
    let result = fx.trash().trash(&file);
    std::fs::set_permissions(fx.work("locked"), std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(result.unwrap_err(), TrashError::PermissionDenied);
    assert!(file.exists());
    assert!(names(&fx.home_trash().join("info")).is_empty());
}

// ------------------------------------------------------------ directorysizes

fn sizes_of(fx: &Fx) -> Vec<SizeEntry> {
    let text = std::fs::read_to_string(fx.home_trash().join("directorysizes")).unwrap_or_default();
    trashinfo::parse_sizes(&text)
}

#[test]
fn trashed_folders_are_recorded_in_directorysizes() {
    let fx = Fx::new();
    write(&fx.work("dir one/a"), "123");
    write(&fx.work("dir one/sub/b"), "12345");
    write(&fx.work("two/c"), "1");
    let first = fx.trash().trash(&fx.work("dir one")).unwrap();
    fx.trash().trash(&fx.work("two")).unwrap();
    write(&fx.work("plain"), "x");
    fx.trash().trash(&fx.work("plain")).unwrap();

    let sizes = sizes_of(&fx);
    assert_eq!(sizes.len(), 2, "only folders are recorded: {sizes:?}");
    let info_mtime = std::fs::metadata(fx.home_trash().join("info/dir%20one.trashinfo"))
        .map(|_| ())
        .is_err();
    assert!(
        info_mtime,
        "names are not percent-encoded on disk, only in directorysizes"
    );
    let one = sizes.iter().find(|entry| entry.name == "dir one").unwrap();
    assert_eq!(one.size, 8);
    let mtime = std::fs::metadata(fx.home_trash().join("info/dir one.trashinfo"))
        .unwrap()
        .mtime();
    assert_eq!(one.mtime, mtime);
    let raw = std::fs::read_to_string(fx.home_trash().join("directorysizes")).unwrap();
    assert!(raw.contains(&format!("8 {mtime} dir%20one\n")), "{raw}");

    let items = fx.trash().list();
    assert_eq!(items.iter().find(|i| i.name == "dir one").unwrap().size, 8);
    assert!(items.iter().find(|i| i.name == "dir one").unwrap().is_dir);

    // Restoring or deleting takes the line out again.
    fx.trash()
        .restore(&first.trash_id, &RestoreTarget::Original)
        .unwrap();
    assert_eq!(sizes_of(&fx).len(), 1);
    let second = fx
        .trash()
        .list()
        .into_iter()
        .find(|i| i.name == "two")
        .unwrap();
    fx.trash().delete(&second.receipt.trash_id).unwrap();
    assert!(!fx.home_trash().join("directorysizes").exists());
}

#[test]
fn a_stale_directorysizes_line_is_recomputed() {
    let fx = Fx::new();
    write(&fx.work("d/a"), "12345");
    fx.trash().trash(&fx.work("d")).unwrap();
    // A wrong size with the wrong mtime is ignored, and a wrong size with the right one is believed.
    let mtime = std::fs::metadata(fx.home_trash().join("info/d.trashinfo"))
        .unwrap()
        .mtime();
    std::fs::write(
        fx.home_trash().join("directorysizes"),
        format!("999 {} d\n", mtime - 5),
    )
    .unwrap();
    assert_eq!(fx.trash().list()[0].size, 5);
    std::fs::write(
        fx.home_trash().join("directorysizes"),
        format!("999 {mtime} d\n"),
    )
    .unwrap();
    assert_eq!(fx.trash().list()[0].size, 999);
    // Garbage lines are skipped.
    std::fs::write(fx.home_trash().join("directorysizes"), "nonsense\n1 2\n\n").unwrap();
    assert_eq!(fx.trash().list()[0].size, 5);
}

// ----------------------------------------------------------------- restoring

#[test]
fn restoring_onto_something_that_exists_is_refused_and_the_item_stays() {
    let fx = Fx::new();
    let file = write(&fx.work("a.txt"), "old");
    let receipt = fx.trash().trash(&file).unwrap();
    write(&file, "new");
    let error = fx
        .trash()
        .restore(&receipt.trash_id, &RestoreTarget::Original)
        .unwrap_err();
    assert_eq!(error, TrashError::OriginExists { path: file.clone() });
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "new",
        "never overwritten"
    );
    assert_eq!(fx.trash().list().len(), 1, "still in the trash");

    // A folder in the way, and a dangling link, count as taken too.
    std::fs::remove_file(&file).unwrap();
    std::fs::create_dir(&file).unwrap();
    assert!(matches!(
        fx.trash()
            .restore(&receipt.trash_id, &RestoreTarget::Original),
        Err(TrashError::OriginExists { .. })
    ));
    std::fs::remove_dir(&file).unwrap();
    symlink("/nowhere", &file).unwrap();
    assert!(matches!(
        fx.trash()
            .restore(&receipt.trash_id, &RestoreTarget::Original),
        Err(TrashError::OriginExists { .. })
    ));
}

#[test]
fn restoring_into_a_folder_that_is_gone_says_which() {
    let fx = Fx::new();
    let file = write(&fx.work("gone/deep/a.txt"), "x");
    let receipt = fx.trash().trash(&file).unwrap();
    std::fs::remove_dir_all(fx.work("gone")).unwrap();
    let error = fx
        .trash()
        .restore(&receipt.trash_id, &RestoreTarget::Original)
        .unwrap_err();
    assert_eq!(
        error,
        TrashError::OriginMissingParent {
            path: fx.work("gone/deep")
        }
    );
    assert!(
        !fx.work("gone").exists(),
        "the plugin never creates the folders"
    );
    // The caller can choose another place.
    let elsewhere = fx.work("elsewhere.txt");
    let restored = fx
        .trash()
        .restore(
            &receipt.trash_id,
            &RestoreTarget::Path {
                path: elsewhere.clone(),
            },
        )
        .unwrap();
    assert_eq!(restored.original_path, elsewhere);
    assert_eq!(std::fs::read_to_string(&elsewhere).unwrap(), "x");
    assert!(fx.trash().list().is_empty());
}

#[test]
fn restoring_to_a_path_checks_that_path_and_stays_on_the_volume() {
    let fx = Fx::new();
    let file = write(&fx.usb.join("a.txt"), "x");
    let receipt = fx.trash().trash(&file).unwrap();
    let taken = write(&fx.usb.join("taken"), "t");
    assert!(matches!(
        fx.trash()
            .restore(&receipt.trash_id, &RestoreTarget::Path { path: taken }),
        Err(TrashError::OriginExists { .. })
    ));
    assert!(matches!(
        fx.trash().restore(
            &receipt.trash_id,
            &RestoreTarget::Path {
                path: fx.work("relative/../x").components().skip(1).collect()
            }
        ),
        Err(TrashError::Io { .. })
    ));
    // Another volume would mean copying, which a restore does not do.
    let error = fx
        .trash()
        .restore(
            &receipt.trash_id,
            &RestoreTarget::Path {
                path: fx.work("moved"),
            },
        )
        .unwrap_err();
    assert!(matches!(error, TrashError::Io { .. }), "{error:?}");
    assert_eq!(fx.trash().list().len(), 1);
    assert!(fx.usb.join(format!(".Trash-{UID}/files/a.txt")).exists());
}

#[test]
fn receipts_that_name_nothing_real_are_not_found() {
    let fx = Fx::new();
    let file = write(&fx.work("a.txt"), "x");
    let receipt = fx.trash().trash(&file).unwrap();
    let root = encode(fx.home_trash().as_os_str().as_encoded_bytes());
    let bad = [
        String::new(),
        "nonsense".to_string(),
        format!("{root}|nothing"),
        format!("{root}|../files/a.txt"),
        format!("{root}|.."),
        format!("{root}|."),
        format!("{root}|"),
        format!("{root}|a%2Fb"),
        // A trash this environment does not know: never reached.
        format!(
            "{}|a.txt",
            encode(fx.work("").as_os_str().as_encoded_bytes())
        ),
        "relative|a.txt".to_string(),
        format!("{}|a.txt", encode(b"/etc")),
    ];
    for id in bad {
        assert_eq!(
            fx.trash()
                .restore(&id, &RestoreTarget::Original)
                .unwrap_err(),
            TrashError::NotFound,
            "{id}"
        );
        assert_eq!(
            fx.trash().delete(&id).unwrap_err(),
            TrashError::NotFound,
            "{id}"
        );
    }
    assert!(fx.home_trash().join("files/a.txt").exists());
    // The real one still works, once.
    fx.trash()
        .restore(&receipt.trash_id, &RestoreTarget::Original)
        .unwrap();
    assert_eq!(
        fx.trash()
            .restore(&receipt.trash_id, &RestoreTarget::Original)
            .unwrap_err(),
        TrashError::NotFound
    );
}

#[test]
fn a_receipt_survives_a_new_instance_like_a_restart() {
    let fx = Fx::new();
    let file = write(&fx.usb.join("a.txt"), "x");
    let receipt = fx.trash().trash(&file).unwrap();
    let again = Freedesktop::with_fs(fx.env.clone(), fx.fs.clone());
    assert_eq!(again.list()[0].receipt.trash_id, receipt.trash_id);
    again
        .restore(&receipt.trash_id, &RestoreTarget::Original)
        .unwrap();
}

#[test]
fn a_restored_link_is_the_same_link() {
    let fx = Fx::new();
    let target = write(&fx.work("target.txt"), "data");
    let link = fx.work("link");
    symlink("target.txt", &link).unwrap();
    let receipt = fx.trash().trash(&link).unwrap();
    assert!(target.exists(), "the target is not trashed");
    assert!(
        std::fs::symlink_metadata(fx.home_trash().join("files/link"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    fx.trash()
        .restore(&receipt.trash_id, &RestoreTarget::Original)
        .unwrap();
    assert_eq!(
        std::fs::read_link(&link).unwrap(),
        PathBuf::from("target.txt")
    );
}

// ----------------------------------------------------------------- what to trash

#[test]
fn a_link_to_a_folder_is_trashed_as_the_link() {
    let fx = Fx::new();
    write(&fx.work("real/inner"), "keep");
    symlink(fx.work("real"), fx.work("alias")).unwrap();
    let receipt = fx.trash().trash(&fx.work("alias")).unwrap();
    assert_eq!(
        std::fs::read_to_string(fx.work("real/inner")).unwrap(),
        "keep"
    );
    let item = fx.trash().list().into_iter().next().unwrap();
    assert!(!item.is_dir, "a link to a folder is not a folder");
    assert_eq!(item.receipt, receipt);
    // With a trailing slash the shell would follow the link; the plugin still trashes the link.
    symlink(fx.work("real"), fx.work("alias2")).unwrap();
    let slash = PathBuf::from(format!("{}/", fx.work("alias2").display()));
    fx.trash().trash(&slash).unwrap();
    assert!(fx.work("real/inner").exists());
    assert!(!fx.work("alias2").exists());
}

#[test]
fn a_dangling_link_is_trashed() {
    let fx = Fx::new();
    symlink("/does/not/exist", fx.work("dangling")).unwrap();
    fx.trash().trash(&fx.work("dangling")).unwrap();
    assert!(std::fs::symlink_metadata(fx.work("dangling")).is_err());
}

#[test]
fn a_trailing_slash_and_dots_in_the_path_are_fine() {
    let fx = Fx::new();
    write(&fx.work("dir/a"), "x");
    let receipt = fx
        .trash()
        .trash(&PathBuf::from(format!("{}/", fx.work("dir").display())))
        .unwrap();
    assert_eq!(receipt.original_path, fx.work("dir"));
    write(&fx.work("f"), "x");
    let odd = PathBuf::from(format!("{}/./sub/../f", fx.work("").display()));
    // `sub` does not exist, so the folder above the file cannot be resolved.
    assert_eq!(fx.trash().trash(&odd).unwrap_err(), TrashError::NotFound);
    std::fs::create_dir(fx.work("sub")).unwrap();
    assert_eq!(fx.trash().trash(&odd).unwrap().original_path, fx.work("f"));
}

#[test]
fn paths_that_name_nothing_are_errors() {
    let fx = Fx::new();
    assert_eq!(
        fx.trash().trash(&fx.work("missing")).unwrap_err(),
        TrashError::NotFound
    );
    assert_eq!(
        fx.trash().trash(&fx.work("missing/deeper")).unwrap_err(),
        TrashError::NotFound
    );
    assert!(matches!(
        fx.trash().trash(Path::new("relative/path")).unwrap_err(),
        TrashError::Io { .. }
    ));
    assert!(matches!(
        fx.trash()
            .trash(&PathBuf::from(format!("{}/..", fx.work("").display())))
            .unwrap_err(),
        TrashError::Refused { .. } | TrashError::Io { .. }
    ));
    assert!(matches!(
        fx.trash().trash(Path::new("/")).unwrap_err(),
        TrashError::Refused { .. }
    ));
}

#[test]
fn what_must_never_be_trashed_is_refused() {
    let fx = Fx::new();
    // Make the trashes exist.
    let file = write(&fx.work("a"), "x");
    fx.trash().trash(&file).unwrap();
    let usb_file = write(&fx.usb.join("b"), "x");
    fx.trash().trash(&usb_file).unwrap();

    let refused = [
        fx.home.clone(),
        fx.root.clone(),
        fx.root.join("mnt"),
        fx.data_home.clone(),
        fx.data_home.parent().unwrap().to_path_buf(),
        fx.home_trash(),
        fx.home_trash().join("files"),
        fx.home_trash().join("files/a"),
        fx.home_trash().join("info/a.trashinfo"),
        fx.usb.clone(),
        fx.disk.clone(),
        fx.usb.join(format!(".Trash-{UID}")),
        fx.usb.join(format!(".Trash-{UID}/files/b")),
        fx.usb.join(".Trash"),
    ];
    for path in refused {
        if !path.exists() {
            // `.Trash` is not made in this test; trashing a path that is not there is NotFound, not a refusal.
            continue;
        }
        let error = fx.trash().trash(&path).unwrap_err();
        assert!(
            matches!(error, TrashError::Refused { .. }),
            "{}: {error:?}",
            path.display()
        );
        assert!(path.exists(), "{} is untouched", path.display());
    }
    // The trash still has what was put in.
    assert_eq!(fx.trash().list().len(), 2);
}

#[test]
fn a_shared_trash_folder_and_its_other_users_are_refused() {
    let fx = Fx::new();
    let shared = fx.usb.join(".Trash");
    std::fs::create_dir_all(shared.join("1001/files")).unwrap();
    for path in [
        shared.clone(),
        shared.join("1001"),
        shared.join("1001/files"),
    ] {
        assert!(matches!(
            fx.trash().trash(&path).unwrap_err(),
            TrashError::Refused { .. }
        ));
    }
}

#[test]
fn the_names_files_and_info_are_not_special_outside_a_trash() {
    let fx = Fx::new();
    let file = write(&fx.work("files"), "x");
    fx.trash().trash(&file).unwrap();
    let info = write(&fx.work("info"), "x");
    fx.trash().trash(&info).unwrap();
    assert_eq!(names(&fx.home_trash().join("files")), ["files", "info"]);
}

// ------------------------------------------------------------------- concurrency

#[test]
fn two_threads_trashing_the_same_name_get_different_slots() {
    let fx = Arc::new(Fx::new());
    let mut paths = Vec::new();
    for index in 0..16 {
        paths.push(write(
            &fx.work(&format!("dir{index}/same.txt")),
            &index.to_string(),
        ));
    }
    let handles: Vec<_> = paths
        .iter()
        .cloned()
        .map(|path| {
            let fx = Arc::clone(&fx);
            std::thread::spawn(move || fx.trash().trash(&path).unwrap())
        })
        .collect();
    let receipts: Vec<TrashReceipt> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let ids: HashSet<&String> = receipts.iter().map(|r| &r.trash_id).collect();
    assert_eq!(ids.len(), 16, "no two trashes took the same name");
    assert_eq!(names(&fx.home_trash().join("files")).len(), 16);
    assert_eq!(names(&fx.home_trash().join("info")).len(), 16);
    for (receipt, path) in receipts.iter().zip(&paths) {
        assert_eq!(&receipt.original_path, path);
        fx.trash()
            .restore(&receipt.trash_id, &RestoreTarget::Original)
            .unwrap();
        let content = std::fs::read_to_string(path).unwrap();
        let index = path
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()[3..]
            .to_string();
        assert_eq!(content, index, "each restored its own content");
    }
}

#[test]
fn threads_trashing_folders_keep_directorysizes_whole() {
    let fx = Arc::new(Fx::new());
    let handles: Vec<_> = (0..12)
        .map(|index| {
            let fx = Arc::clone(&fx);
            let dir = fx.work(&format!("d{index}"));
            write(&dir.join("f"), "12");
            std::thread::spawn(move || fx.trash().trash(&dir).unwrap())
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
    assert_eq!(sizes_of(&fx).len(), 12);
    assert!(names(&fx.home_trash())
        .iter()
        .all(|name| !name.ends_with(".tmp")));
}

// ---------------------------------------------------------------- delete and empty

#[test]
fn deleting_an_item_removes_it_and_its_trashinfo() {
    let fx = Fx::new();
    write(&fx.work("tree/locked/inner/file"), "x");
    std::fs::set_permissions(
        fx.work("tree/locked/inner"),
        std::fs::Permissions::from_mode(0o500),
    )
    .unwrap();
    std::fs::set_permissions(
        fx.work("tree/locked"),
        std::fs::Permissions::from_mode(0o500),
    )
    .unwrap();
    let receipt = fx.trash().trash(&fx.work("tree")).unwrap();
    fx.trash().delete(&receipt.trash_id).unwrap();
    assert!(names(&fx.home_trash().join("files")).is_empty());
    assert!(names(&fx.home_trash().join("info")).is_empty());
    assert_eq!(
        fx.trash().delete(&receipt.trash_id).unwrap_err(),
        TrashError::NotFound
    );
}

#[test]
fn deleting_a_link_leaves_what_it_points_at() {
    let fx = Fx::new();
    write(&fx.work("real/f"), "keep");
    symlink(fx.work("real"), fx.work("alias")).unwrap();
    let receipt = fx.trash().trash(&fx.work("alias")).unwrap();
    fx.trash().delete(&receipt.trash_id).unwrap();
    assert_eq!(std::fs::read_to_string(fx.work("real/f")).unwrap(), "keep");
}

#[test]
fn deleting_an_orphan_trashinfo_cleans_it() {
    let fx = Fx::new();
    let file = write(&fx.work("a"), "x");
    let receipt = fx.trash().trash(&file).unwrap();
    std::fs::remove_file(fx.home_trash().join("files/a")).unwrap();
    assert!(fx.trash().list().is_empty(), "an orphan is not listed");
    fx.trash().delete(&receipt.trash_id).unwrap();
    assert!(names(&fx.home_trash().join("info")).is_empty());
}

#[test]
fn emptying_everything_clears_every_trash_and_the_leftovers() {
    let fx = Fx::new();
    write(&fx.work("dir/f"), "x");
    fx.trash().trash(&fx.work("dir")).unwrap();
    fx.trash().trash(&write(&fx.work("file"), "x")).unwrap();
    fx.trash().trash(&write(&fx.usb.join("u"), "x")).unwrap();
    fx.trash().trash(&write(&fx.disk.join("d"), "x")).unwrap();
    // Leftovers: an item without a `.trashinfo` and a `.trashinfo` without an item.
    write(&fx.home_trash().join("files/stray"), "x");
    write(
        &fx.home_trash().join("info/lost.trashinfo"),
        "[Trash Info]\nPath=/x\n",
    );

    let report = fx.trash().empty(None);
    assert!(report.failed.is_empty(), "{report:?}");
    assert_eq!(report.removed, 5);
    assert!(fx.trash().list().is_empty());
    for trash in [
        fx.home_trash(),
        fx.usb.join(format!(".Trash-{UID}")),
        fx.disk.join(format!(".Trash-{UID}")),
    ] {
        assert!(
            names(&trash.join("files")).is_empty(),
            "{}",
            trash.display()
        );
        assert!(names(&trash.join("info")).is_empty(), "{}", trash.display());
        assert!(trash.join("files").is_dir(), "the trash itself stays");
    }
    assert!(!fx.home_trash().join("directorysizes").exists());
    // Emptying an empty trash is fine.
    assert_eq!(fx.trash().empty(None), EmptyReport::default());
}

#[test]
fn emptying_removes_the_item_before_its_trashinfo() {
    if is_root_user() {
        return;
    }
    let fx = Fx::new();
    fx.trash().trash(&write(&fx.work("a"), "x")).unwrap();
    fx.trash().trash(&write(&fx.work("b"), "x")).unwrap();
    // With `files/` read-only no item can be removed: the `.trashinfo` files must then stay, so every item is still listed with its origin.
    let files = fx.home_trash().join("files");
    std::fs::set_permissions(&files, std::fs::Permissions::from_mode(0o500)).unwrap();
    let report = fx.trash().empty(None);
    std::fs::set_permissions(&files, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(report.removed, 0);
    assert_eq!(report.failed.len(), 2);
    assert!(report
        .failed
        .iter()
        .all(|failure| failure.error == TrashError::PermissionDenied));
    assert_eq!(names(&fx.home_trash().join("info")).len(), 2);
    assert_eq!(fx.trash().list().len(), 2);
    // And once it can, it works.
    assert_eq!(fx.trash().empty(None).removed, 2);
}

#[test]
fn emptying_by_age_removes_only_old_items() {
    let fx = Fx::new();
    let old = write(&fx.work("old"), "x");
    let older_dir = write(&fx.usb.join("older/f"), "x");
    fx.trash().trash(&old).unwrap();
    fx.trash().trash(&fx.usb.join("older")).unwrap();
    fx.advance(Duration::days(10));
    let recent = write(&fx.work("recent"), "x");
    fx.trash().trash(&recent).unwrap();
    fx.advance(Duration::days(20));
    // old: 30 days, older: 30 days, recent: 20 days.
    let report = fx.trash().empty(Some(25));
    assert_eq!(report.removed, 2);
    let left = fx.trash().list();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].original_path, recent);
    assert!(!older_dir.exists());
    assert!(!fx.usb.join(format!(".Trash-{UID}/files/older")).exists());
    assert!(!fx
        .usb
        .join(format!(".Trash-{UID}/info/older.trashinfo"))
        .exists());
    assert!(!fx.usb.join(format!(".Trash-{UID}/directorysizes")).exists());

    // Exactly N days old counts as old; one second short does not.
    // `recent` is 20 days old now; take one second off.
    fx.advance(Duration::seconds(-1));
    assert_eq!(fx.trash().empty(Some(20)).removed, 0);
    assert_eq!(fx.trash().list().len(), 1);
    fx.advance(Duration::seconds(1));
    assert_eq!(fx.trash().empty(Some(20)).removed, 1);
}

#[test]
fn zero_days_means_everything_trashed_so_far() {
    let fx = Fx::new();
    fx.trash().trash(&write(&fx.work("a"), "x")).unwrap();
    assert_eq!(fx.trash().empty(Some(0)).removed, 1);
}

#[test]
fn a_symlinked_files_folder_is_never_followed() {
    let fx = Fx::new();
    let victim = fx.root.join("victim");
    write(&victim.join("precious"), "keep");
    let trash = fx.home_trash();
    private_dirs(&trash.join("info"));
    symlink(&victim, trash.join("files")).unwrap();
    write(
        &trash.join("info/precious.trashinfo"),
        "[Trash Info]\nPath=/x\nDeletionDate=2000-01-01T00:00:00\n",
    );
    assert!(fx.trash().list().is_empty());
    assert_eq!(fx.trash().empty(None), EmptyReport::default());
    assert_eq!(fx.trash().empty(Some(0)), EmptyReport::default());
    assert!(victim.join("precious").exists());
    // Trashing into such a trash fails instead of following the link.
    let file = write(&fx.work("a"), "x");
    assert!(fx.trash().trash(&file).is_err());
    assert!(file.exists());
    assert!(!victim.join("a").exists());
}

// ----------------------------------------------------------------- listing

#[test]
fn unreadable_and_orphaned_entries_are_skipped_not_fatal() {
    let fx = Fx::new();
    fx.trash().trash(&write(&fx.work("good"), "x")).unwrap();
    let trash = fx.home_trash();
    write(&trash.join("files/bad"), "x");
    write(&trash.join("info/bad.trashinfo"), "garbage with no group");
    write(
        &trash.join("info/orphan.trashinfo"),
        "[Trash Info]\nPath=/o\nDeletionDate=2026-01-01T00:00:00\n",
    );
    write(&trash.join("info/not-info.txt"), "x");
    write(&trash.join("info/.trashinfo"), "[Trash Info]\nPath=/o\n");
    let items = fx.trash().list();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "good");
}

#[test]
fn an_item_shows_the_name_it_had_not_the_one_the_trash_gave_it() {
    let fx = Fx::new();
    fx.trash()
        .trash(&write(&fx.work("a/doc.txt"), "x"))
        .unwrap();
    fx.trash()
        .trash(&write(&fx.work("b/doc.txt"), "x"))
        .unwrap();
    let items = fx.trash().list();
    assert_eq!(items.len(), 2);
    assert!(items.iter().all(|item| item.name == "doc.txt"));
}

// ------------------------------------------------------------------ the real env

#[test]
fn mountinfo_is_parsed_with_escapes_and_pseudo_filesystems_dropped() {
    let text = "\
22 1 259:2 / / rw,relatime shared:1 - ext4 /dev/nvme0n1p2 rw
23 22 0:5 / /proc rw,nosuid shared:2 - proc proc rw
24 22 0:21 / /sys rw shared:3 - sysfs sysfs rw
30 22 8:1 / /run/media/user/My\\040Disk rw,nosuid - vfat /dev/sda1 rw
31 22 0:40 / /mnt/share rw - nfs4 server:/export rw
32 22 0:41 / /run/user/1000/doc rw - fuse.portal portal rw
33 22 0:42 / /mnt/ssh rw - fuse.sshfs user@host:/ rw
34 22 0:43 / /mnt/tmp rw shared:4 master:5 - tmpfs tmpfs rw
broken line
";
    let mounts = parse_mountinfo(text);
    let points: Vec<&Path> = mounts.iter().map(|m| m.mount_point.as_path()).collect();
    assert_eq!(
        points,
        [
            Path::new("/"),
            Path::new("/run/media/user/My Disk"),
            Path::new("/mnt/share"),
            Path::new("/mnt/ssh"),
            Path::new("/mnt/tmp"),
        ]
    );
    assert!(mounts[1].is_removable && !mounts[1].is_network);
    assert!(mounts[2].is_network && mounts[3].is_network);
    assert!(!mounts[0].is_removable);
}

#[test]
fn the_default_environment_reads_something_plausible() {
    // Only reads: nothing is created, and no trash is touched.
    let env = default_env();
    assert!(env.data_home.is_absolute());
    assert!(env
        .mounts
        .iter()
        .any(|mount| mount.mount_point == Path::new("/")));
}

// ------------------------------------------------------------- property test

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

const PIECES: &[&[u8]] = &[
    b"a",
    b"b",
    b"x y",
    b"%",
    b"#",
    b"\n",
    "é".as_bytes(),
    b"\xff",
    b".",
    b"..tail",
    b"-",
    b"[]",
    b"Path=",
    b"%41",
];

fn random_name(rng: &mut Rng) -> OsString {
    loop {
        let mut bytes = Vec::new();
        for _ in 0..1 + rng.below(3) {
            bytes.extend_from_slice(PIECES[rng.below(PIECES.len() as u64) as usize]);
        }
        if bytes != b"." && bytes != b".." {
            return bytes_name(&bytes);
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Node {
    File(Vec<u8>),
    Link(PathBuf),
    Dir,
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, Node> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Node>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            let relative = path.strip_prefix(root).unwrap().to_path_buf();
            let metadata = std::fs::symlink_metadata(&path).unwrap();
            if metadata.file_type().is_symlink() {
                out.insert(relative, Node::Link(std::fs::read_link(&path).unwrap()));
            } else if metadata.is_dir() {
                out.insert(relative, Node::Dir);
                walk(root, &path, out);
            } else {
                out.insert(relative, Node::File(std::fs::read(&path).unwrap()));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn grow(rng: &mut Rng, dir: &Path, depth: u32) {
    for _ in 0..1 + rng.below(4) {
        let path = dir.join(random_name(rng));
        if std::fs::symlink_metadata(&path).is_ok() {
            continue;
        }
        match rng.below(4) {
            0 if depth < 3 => {
                std::fs::create_dir(&path).unwrap();
                grow(rng, &path, depth + 1);
            }
            1 => symlink(random_name(rng), &path).unwrap(),
            _ => {
                let content: Vec<u8> = (0..rng.below(20)).map(|_| rng.below(256) as u8).collect();
                std::fs::write(&path, content).unwrap();
            }
        }
    }
}

#[test]
fn random_trees_trashed_then_restored_are_unchanged() {
    for seed in 1..=30u64 {
        let fx = Fx::new();
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        // Two folders with names drawn from the same small set, so names collide in the trash; one on a second volume.
        let places = [fx.work("p"), fx.work("q"), fx.usb.join("u")];
        for place in &places {
            std::fs::create_dir_all(place).unwrap();
            grow(&mut rng, place, 0);
        }
        let before: Vec<_> = places.iter().map(|place| snapshot(place)).collect();

        let mut targets: Vec<PathBuf> = places
            .iter()
            .flat_map(|place| {
                std::fs::read_dir(place)
                    .unwrap()
                    .flatten()
                    .map(|entry| entry.path())
                    .collect::<Vec<_>>()
            })
            .collect();
        // Trash in a random order.
        for index in (1..targets.len()).rev() {
            targets.swap(index, rng.below(index as u64 + 1) as usize);
        }
        let mut receipts = Vec::new();
        for target in &targets {
            receipts.push((target.clone(), fx.trash().trash(target).unwrap()));
        }
        for place in &places {
            assert!(
                snapshot(place).is_empty(),
                "seed {seed}: {place:?} not emptied"
            );
        }
        let listed = fx.trash().list();
        assert_eq!(listed.len(), targets.len(), "seed {seed}");
        let listed_paths: HashSet<PathBuf> =
            listed.iter().map(|i| i.original_path.clone()).collect();
        assert_eq!(
            listed_paths,
            targets.iter().cloned().collect::<HashSet<_>>(),
            "seed {seed}"
        );

        for (target, receipt) in receipts.iter().rev() {
            let restored = fx
                .trash()
                .restore(&receipt.trash_id, &RestoreTarget::Original)
                .unwrap_or_else(|error| panic!("seed {seed}: {target:?}: {error:?}"));
            assert_eq!(&restored.original_path, target);
        }
        let after: Vec<_> = places.iter().map(|place| snapshot(place)).collect();
        assert_eq!(before, after, "seed {seed}");
        assert!(fx.trash().list().is_empty(), "seed {seed}");
    }
}
