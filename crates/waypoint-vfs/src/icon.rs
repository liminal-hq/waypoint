// Chooses the bundled icon group for an entry from its kind, name and (where known) MIME type.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{EntryKind, IconGroup};

/// The extension of a name as bytes (without the dot), or empty. A leading dot alone (`.bashrc`)
/// is part of the name, not an extension.
pub(crate) fn extension(name: &[u8]) -> &[u8] {
    match name.iter().rposition(|&b| b == b'.') {
        Some(at) if at > 0 && at + 1 < name.len() => &name[at + 1..],
        _ => &[],
    }
}

/// The icon group for a name. Folders (and links to folders) are decided by `kind`; a link to
/// something that is neither a file nor a folder is a `Symlink`; everything else is decided by
/// the whole name (a few well-known ones such as `Makefile`) and then the extension, compared
/// without regard to case. Files the tables do not know are `Other`.
pub fn group_for(name: &[u8], kind: EntryKind, link_target: Option<EntryKind>) -> IconGroup {
    if kind == EntryKind::Directory || link_target == Some(EntryKind::Directory) {
        return IconGroup::Folder;
    }
    if matches!(link_target, Some(EntryKind::Symlink | EntryKind::Other)) {
        return IconGroup::Symlink;
    }
    if let Some(group) = by_name(name) {
        return group;
    }
    let mut ext = [0u8; 8];
    let raw = extension(name);
    if raw.is_empty() || raw.len() > ext.len() {
        return IconGroup::Other;
    }
    let ext = &mut ext[..raw.len()];
    ext.copy_from_slice(raw);
    ext.make_ascii_lowercase();
    by_extension(ext)
}

/// The group for an entry a scan has just read: `group_for`, plus what only the scan knows.
///
/// - A symlink that has been resolved and points at nothing (a broken link) is a `Symlink`. One the
///   scan has not resolved yet (`link_pending`) is grouped by its name, as a link to a file is.
/// - A file with the execute bit that the name tables leave as `Other` is an `Executable`. The bit
///   comes from the metadata the directory read already gave, so this costs no extra call, and no
///   content is sniffed. A `.txt` with the bit set (common on FAT and network mounts) stays text.
pub fn group_for_scan(
    name: &[u8],
    kind: EntryKind,
    link_target: Option<EntryKind>,
    link_pending: bool,
    executable: bool,
) -> IconGroup {
    if kind == EntryKind::Symlink && link_target.is_none() && !link_pending {
        return IconGroup::Symlink;
    }
    let group = group_for(name, kind, link_target);
    let is_file = kind == EntryKind::File
        || (kind == EntryKind::Symlink && link_target == Some(EntryKind::File));
    if group == IconGroup::Other && executable && is_file {
        IconGroup::Executable
    } else {
        group
    }
}

/// A few names that say what a file is better than their extension does (or have none).
const BY_NAME: &[(&str, IconGroup)] = &[
    ("makefile", IconGroup::Code),
    ("gnumakefile", IconGroup::Code),
    ("dockerfile", IconGroup::Code),
    ("containerfile", IconGroup::Code),
    ("justfile", IconGroup::Code),
    ("rakefile", IconGroup::Code),
    ("gemfile", IconGroup::Code),
    ("vagrantfile", IconGroup::Code),
    ("jenkinsfile", IconGroup::Code),
    ("procfile", IconGroup::Code),
    ("cmakelists.txt", IconGroup::Code),
    ("license", IconGroup::Text),
    ("licence", IconGroup::Text),
    ("copying", IconGroup::Text),
    ("readme", IconGroup::Text),
    ("notice", IconGroup::Text),
    ("authors", IconGroup::Text),
    ("changelog", IconGroup::Text),
    (".gitignore", IconGroup::Config),
    (".gitattributes", IconGroup::Config),
    (".gitmodules", IconGroup::Config),
    (".editorconfig", IconGroup::Config),
    (".npmrc", IconGroup::Config),
    (".env", IconGroup::Config),
    (".bashrc", IconGroup::ShellScript),
    (".bash_profile", IconGroup::ShellScript),
    (".bash_logout", IconGroup::ShellScript),
    (".zshrc", IconGroup::ShellScript),
    (".zprofile", IconGroup::ShellScript),
    (".zshenv", IconGroup::ShellScript),
    (".profile", IconGroup::ShellScript),
    ("id_rsa", IconGroup::Certificate),
    ("id_dsa", IconGroup::Certificate),
    ("id_ecdsa", IconGroup::Certificate),
    ("id_ed25519", IconGroup::Certificate),
];

fn by_name(name: &[u8]) -> Option<IconGroup> {
    // The longest known name is 14 bytes, so most names are turned away on length alone.
    if name.len() > 14 {
        return None;
    }
    BY_NAME
        .iter()
        .find(|(known, _)| known.as_bytes().eq_ignore_ascii_case(name))
        .map(|(_, group)| *group)
}

/// The group of a lower-case extension.
fn by_extension(ext: &[u8]) -> IconGroup {
    use IconGroup::*;
    match ext {
        b"png" | b"jpg" | b"jpeg" | b"jpe" | b"gif" | b"webp" | b"bmp" | b"svg" | b"svgz"
        | b"tif" | b"tiff" | b"ico" | b"icns" | b"heic" | b"heif" | b"avif" | b"jxl" | b"cr2"
        | b"cr3" | b"nef" | b"arw" | b"dng" | b"raw" | b"orf" | b"rw2" | b"psd" | b"xcf"
        | b"kra" | b"exr" | b"hdr" => Image,
        b"mp3" | b"flac" | b"ogg" | b"oga" | b"opus" | b"wav" | b"m4a" | b"aac" | b"wma"
        | b"aiff" | b"aif" | b"mka" | b"mid" | b"midi" | b"amr" | b"ape" | b"wv" => Audio,
        b"mp4" | b"mkv" | b"webm" | b"avi" | b"mov" | b"wmv" | b"m4v" | b"mpg" | b"mpeg"
        | b"flv" | b"3gp" | b"ogv" | b"m2ts" | b"mts" | b"vob" => Video,
        b"zip" | b"tar" | b"gz" | b"tgz" | b"bz2" | b"tbz2" | b"xz" | b"txz" | b"zst" | b"7z"
        | b"rar" | b"lz" | b"lzma" | b"z" | b"cab" | b"cpio" | b"jar" | b"war" => Archive,
        b"iso" | b"img" | b"dmg" | b"vhd" | b"vhdx" | b"vmdk" | b"qcow" | b"qcow2" | b"vdi"
        | b"toast" => DiskImage,
        b"rs" | b"ts" | b"tsx" | b"cts" | b"js" | b"jsx" | b"mjs" | b"cjs" | b"py" | b"pyw"
        | b"c" | b"h" | b"cc" | b"cpp" | b"cxx" | b"hpp" | b"hxx" | b"go" | b"java" | b"kt"
        | b"kts" | b"cs" | b"rb" | b"php" | b"lua" | b"css" | b"scss" | b"sass" | b"less"
        | b"html" | b"htm" | b"xhtml" | b"sql" | b"swift" | b"dart" | b"scala" | b"zig" | b"hs"
        | b"ml" | b"ex" | b"exs" | b"erl" | b"clj" | b"r" | b"jl" | b"pl" | b"pm" | b"vue"
        | b"svelte" | b"astro" | b"asm" | b"s" | b"gradle" | b"ipynb" => Code,
        b"sh" | b"bash" | b"zsh" | b"fish" | b"ksh" | b"bat" | b"cmd" | b"ps1" => ShellScript,
        b"json" | b"jsonc" | b"json5" | b"toml" | b"yaml" | b"yml" | b"xml" | b"ini" | b"conf"
        | b"cfg" | b"properties" | b"env" | b"editorconfig" | b"plist" | b"reg" | b"service"
        | b"lock" => Config,
        b"doc" | b"docx" | b"odt" | b"rtf" | b"dot" | b"dotx" | b"pages" | b"wpd" | b"abw" => {
            Document
        }
        b"pdf" => Pdf,
        b"txt" | b"text" | b"nfo" | b"rst" | b"org" | b"adoc" => Text,
        b"md" | b"markdown" | b"mdx" | b"mkd" => Markdown,
        b"xls" | b"xlsx" | b"xlsm" | b"xlsb" | b"ods" | b"numbers" | b"csv" | b"tsv" => Spreadsheet,
        b"ppt" | b"pptx" | b"pps" | b"ppsx" | b"odp" | b"keynote" => Presentation,
        b"ttf" | b"otf" | b"woff" | b"woff2" | b"eot" | b"ttc" | b"pfb" | b"pfm" | b"fon"
        | b"bdf" | b"pcf" => Font,
        b"db" | b"sqlite" | b"sqlite3" | b"db3" | b"mdb" | b"accdb" | b"dbf" | b"kdbx" => Database,
        b"pem" | b"crt" | b"cer" | b"der" | b"p12" | b"pfx" | b"key" | b"pub" | b"gpg" | b"pgp"
        | b"asc" | b"sig" | b"csr" | b"jks" | b"keystore" | b"p7b" | b"p7c" => Certificate,
        b"epub" | b"mobi" | b"azw" | b"azw3" | b"fb2" | b"djvu" | b"cbz" | b"cbr" => Ebook,
        b"torrent" | b"magnet" => Torrent,
        b"ics" | b"ical" | b"icalendar" | b"ifb" => Calendar,
        b"vcf" | b"vcard" => Contact,
        b"log" => Log,
        b"stl" | b"obj" | b"fbx" | b"gltf" | b"glb" | b"blend" | b"3ds" | b"dae" | b"step"
        | b"stp" | b"iges" | b"igs" | b"3mf" | b"ply" | b"x3d" => Model3d,
        b"srt" | b"vtt" | b"ass" | b"ssa" | b"sub" | b"idx" | b"sup" | b"lrc" => Subtitles,
        b"m3u" | b"m3u8" | b"pls" | b"xspf" | b"wpl" | b"asx" | b"cue" => Playlist,
        b"deb" | b"rpm" | b"flatpak" | b"flatpakref" | b"snap" | b"apk" | b"aab" | b"msi"
        | b"msix" | b"appx" | b"pkg" | b"whl" | b"nupkg" | b"gem" | b"crate" | b"vsix" | b"xpi"
        | b"crx" => Package,
        b"desktop" | b"appimage" | b"exe" => App,
        b"bin" | b"run" | b"elf" | b"out" | b"com" | b"so" | b"dll" | b"dylib" => Executable,
        _ => Other,
    }
}

/// The group for a MIME type, where one is known, for a provider that reports types rather than
/// names. Parameters (`; charset=utf-8`) and case are ignored. `None` means "not sure": the caller
/// falls back to `group_for` on the name.
pub fn group_for_mime(mime: &str) -> Option<IconGroup> {
    use IconGroup::*;
    let essence = mime
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let mime = essence.as_str();
    let exact = match mime {
        "inode/directory" => Folder,
        "inode/symlink" => Symlink,
        "application/pdf" => Pdf,
        "text/plain" => Text,
        "text/markdown" | "text/x-markdown" => Markdown,
        "text/csv"
        | "text/tab-separated-values"
        | "application/vnd.ms-excel"
        | "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        | "application/vnd.oasis.opendocument.spreadsheet" => Spreadsheet,
        "application/vnd.ms-powerpoint"
        | "application/vnd.openxmlformats-officedocument.presentationml.presentation"
        | "application/vnd.oasis.opendocument.presentation" => Presentation,
        "application/msword"
        | "application/rtf"
        | "text/rtf"
        | "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        | "application/vnd.oasis.opendocument.text" => Document,
        "application/zip"
        | "application/gzip"
        | "application/x-tar"
        | "application/x-7z-compressed"
        | "application/vnd.rar"
        | "application/x-rar-compressed"
        | "application/x-bzip2"
        | "application/x-xz"
        | "application/zstd"
        | "application/java-archive" => Archive,
        "application/x-iso9660-image"
        | "application/x-raw-disk-image"
        | "application/x-apple-diskimage" => DiskImage,
        "application/vnd.sqlite3" | "application/x-sqlite3" | "application/vnd.ms-access" => {
            Database
        }
        "application/json" | "application/yaml" | "application/x-yaml" | "text/yaml"
        | "application/toml" | "application/xml" | "text/xml" | "text/x-ini" => Config,
        "application/x-shellscript" | "text/x-shellscript" | "application/x-sh" => ShellScript,
        "application/x-desktop"
        | "application/x-appimage"
        | "application/vnd.appimage"
        | "application/vnd.microsoft.portable-executable"
        | "application/x-msdownload"
        | "application/x-ms-dos-executable" => App,
        "application/x-executable"
        | "application/x-pie-executable"
        | "application/x-sharedlib"
        | "application/x-mach-binary"
        | "application/x-object" => Executable,
        "application/x-pem-file"
        | "application/pkix-cert"
        | "application/x-x509-ca-cert"
        | "application/x-x509-user-cert"
        | "application/pgp-keys"
        | "application/pgp-signature"
        | "application/x-pkcs12"
        | "application/pkcs12"
        | "application/pkcs10" => Certificate,
        "application/epub+zip"
        | "application/x-mobipocket-ebook"
        | "application/vnd.amazon.ebook"
        | "image/vnd.djvu"
        | "application/x-cbz"
        | "application/vnd.comicbook+zip" => Ebook,
        "application/x-bittorrent" => Torrent,
        "text/calendar" => Calendar,
        "text/vcard" | "text/x-vcard" | "text/directory" => Contact,
        "application/x-subrip" | "text/vtt" | "text/x-ssa" | "application/x-ass" => Subtitles,
        "audio/x-mpegurl"
        | "audio/mpegurl"
        | "application/vnd.apple.mpegurl"
        | "audio/x-scpls"
        | "application/pls+xml"
        | "application/xspf+xml"
        | "application/vnd.ms-wpl" => Playlist,
        "application/vnd.debian.binary-package"
        | "application/x-deb"
        | "application/x-rpm"
        | "application/vnd.flatpak"
        | "application/vnd.flatpak.ref"
        | "application/vnd.snap"
        | "application/vnd.android.package-archive"
        | "application/x-msi"
        | "application/x-ms-installer"
        | "application/x-wheel+zip"
        | "application/vnd.microsoft.appx" => Package,
        "application/font-woff"
        | "application/x-font-ttf"
        | "application/x-font-otf"
        | "application/vnd.ms-fontobject" => Font,
        "application/javascript"
        | "application/x-javascript"
        | "application/typescript"
        | "application/x-php"
        | "application/x-perl"
        | "application/x-python"
        | "application/x-ruby"
        | "application/sql"
        | "application/x-httpd-php" => Code,
        _ => Other,
    };
    if exact != Other {
        return Some(exact);
    }
    let (top, sub) = mime.split_once('/')?;
    match top {
        "image" => Some(Image),
        "audio" => Some(Audio),
        "video" => Some(Video),
        "font" => Some(Font),
        "model" => Some(Model3d),
        "text" if sub.starts_with("x-") || matches!(sub, "html" | "css" | "javascript") => {
            Some(Code)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: EntryKind = EntryKind::File;

    fn of(name: &str) -> IconGroup {
        group_for(name.as_bytes(), FILE, None)
    }

    #[test]
    fn finds_the_extension_but_not_a_leading_dot() {
        assert_eq!(extension(b"a.tar.gz"), b"gz");
        assert_eq!(extension(b".bashrc"), b"");
        assert_eq!(extension(b"name."), b"");
        assert_eq!(extension(b"noext"), b"");
    }

    #[test]
    fn groups_by_extension_without_regard_to_case() {
        assert_eq!(of("a.PNG"), IconGroup::Image);
        assert_eq!(of("a.Flac"), IconGroup::Audio);
        assert_eq!(of("a.mkv"), IconGroup::Video);
        assert_eq!(of("a.tar.ZST"), IconGroup::Archive);
        assert_eq!(of("lib.rs"), IconGroup::Code);
        assert_eq!(of("notes.MD"), IconGroup::Markdown);
        assert_eq!(of("a.unknown"), IconGroup::Other);
        assert_eq!(of("a.averyveryverylongextension"), IconGroup::Other);
        assert_eq!(of("noextension"), IconGroup::Other);
    }

    #[test]
    fn every_group_has_a_file_that_lands_in_it() {
        use IconGroup::*;
        let samples: &[(&str, IconGroup)] = &[
            ("a.jpg", Image),
            ("a.doc", Document),
            ("a.pdf", Pdf),
            ("a.py", Code),
            ("a.zip", Archive),
            ("a.opus", Audio),
            ("a.webm", Video),
            ("a.AppImage", App),
            ("setup.exe", App),
            ("a.desktop", App),
            ("a.txt", Text),
            ("a.md", Markdown),
            ("a.xlsx", Spreadsheet),
            ("a.odp", Presentation),
            ("a.woff2", Font),
            ("a.iso", DiskImage),
            ("a.sqlite3", Database),
            ("a.yaml", Config),
            ("a.sh", ShellScript),
            ("a.bin", Executable),
            ("a.pem", Certificate),
            ("a.epub", Ebook),
            ("a.torrent", Torrent),
            ("a.ics", Calendar),
            ("a.vcf", Contact),
            ("a.log", Log),
            ("a.stl", Model3d),
            ("a.srt", Subtitles),
            ("a.m3u8", Playlist),
            ("a.deb", Package),
            ("a.xyz", Other),
        ];
        for (name, group) in samples {
            assert_eq!(of(name), *group, "{name}");
        }
        let mut seen: Vec<IconGroup> = samples.iter().map(|(_, g)| *g).collect();
        seen.extend([IconGroup::Folder, IconGroup::Symlink]);
        for group in IconGroup::ALL {
            assert!(seen.contains(&group), "{group:?} has no sample");
        }
    }

    #[test]
    fn well_known_names_win_over_their_extension() {
        assert_eq!(of("Makefile"), IconGroup::Code);
        assert_eq!(of("DOCKERFILE"), IconGroup::Code);
        assert_eq!(of("CMakeLists.txt"), IconGroup::Code);
        assert_eq!(of("LICENSE"), IconGroup::Text);
        assert_eq!(of(".gitignore"), IconGroup::Config);
        assert_eq!(of(".bashrc"), IconGroup::ShellScript);
        assert_eq!(of("id_ed25519"), IconGroup::Certificate);
        assert_eq!(of("id_ed25519.pub"), IconGroup::Certificate);
        assert_eq!(of("makefile.bak"), IconGroup::Other);
    }

    #[test]
    fn folders_and_links_to_folders_get_the_folder_group() {
        assert_eq!(
            group_for(b"x.png", EntryKind::Directory, None),
            IconGroup::Folder
        );
        assert_eq!(
            group_for(b"x.png", EntryKind::Symlink, Some(EntryKind::Directory)),
            IconGroup::Folder
        );
        assert_eq!(
            group_for(b"x.png", EntryKind::Symlink, Some(EntryKind::File)),
            IconGroup::Image
        );
        assert_eq!(
            group_for(b"x.png", EntryKind::Symlink, Some(EntryKind::Other)),
            IconGroup::Symlink
        );
    }

    #[test]
    fn a_scan_marks_broken_links_but_not_pending_ones() {
        let link = EntryKind::Symlink;
        assert_eq!(
            group_for_scan(b"x.png", link, None, false, false),
            IconGroup::Symlink
        );
        assert_eq!(
            group_for_scan(b"x.png", link, None, true, false),
            IconGroup::Image
        );
        assert_eq!(
            group_for_scan(b"x.png", link, Some(EntryKind::File), false, false),
            IconGroup::Image
        );
    }

    #[test]
    fn the_execute_bit_only_decides_files_the_tables_do_not_know() {
        assert_eq!(
            group_for_scan(b"tool", FILE, None, false, true),
            IconGroup::Executable
        );
        assert_eq!(
            group_for_scan(b"tool", FILE, None, false, false),
            IconGroup::Other
        );
        assert_eq!(
            group_for_scan(b"notes.txt", FILE, None, false, true),
            IconGroup::Text
        );
        assert_eq!(
            group_for_scan(b"run.sh", FILE, None, false, true),
            IconGroup::ShellScript
        );
        assert_eq!(
            group_for_scan(b"dir", EntryKind::Directory, None, false, true),
            IconGroup::Folder
        );
    }

    #[test]
    fn mime_types_map_exactly_then_by_family() {
        use IconGroup::*;
        assert_eq!(group_for_mime("application/pdf"), Some(Pdf));
        assert_eq!(group_for_mime("Text/Plain; charset=utf-8"), Some(Text));
        assert_eq!(group_for_mime("text/markdown"), Some(Markdown));
        assert_eq!(group_for_mime("image/png"), Some(Image));
        assert_eq!(group_for_mime("audio/flac"), Some(Audio));
        assert_eq!(group_for_mime("video/mp4"), Some(Video));
        assert_eq!(group_for_mime("font/woff2"), Some(Font));
        assert_eq!(group_for_mime("model/stl"), Some(Model3d));
        assert_eq!(group_for_mime("text/x-rust"), Some(Code));
        assert_eq!(group_for_mime("application/zip"), Some(Archive));
        assert_eq!(
            group_for_mime("application/x-shellscript"),
            Some(ShellScript)
        );
        assert_eq!(group_for_mime("application/x-sharedlib"), Some(Executable));
        assert_eq!(group_for_mime("application/x-bittorrent"), Some(Torrent));
        assert_eq!(group_for_mime("inode/directory"), Some(Folder));
        assert_eq!(group_for_mime("application/octet-stream"), None);
        assert_eq!(group_for_mime("nonsense"), None);
    }

    #[test]
    fn refined_groups_keep_the_coarse_kind_they_had() {
        use IconGroup::*;
        assert_eq!(Pdf.kind_class(), Document);
        assert_eq!(Spreadsheet.kind_class(), Document);
        assert_eq!(Config.kind_class(), Code);
        assert_eq!(ShellScript.kind_class(), Code);
        assert_eq!(DiskImage.kind_class(), Archive);
        assert_eq!(Playlist.kind_class(), Audio);
        assert_eq!(Executable.kind_class(), Other);
        for group in IconGroup::ALL {
            assert!((group.kind_class() as u8) < 8, "{group:?}");
            assert_eq!(group.kind_class().kind_class(), group.kind_class());
        }
    }

    #[test]
    fn the_all_list_is_complete_and_in_order() {
        for (index, group) in IconGroup::ALL.iter().enumerate() {
            assert_eq!(*group as usize, index);
        }
    }
}
