// Chooses the bundled icon group for an entry from its kind and extension.
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

/// The icon group for a name. Folders (and links to folders) are decided by `kind`, everything else
/// by the extension, compared without regard to case.
pub fn group_for(name: &[u8], kind: EntryKind, link_target: Option<EntryKind>) -> IconGroup {
    if kind == EntryKind::Directory || link_target == Some(EntryKind::Directory) {
        return IconGroup::Folder;
    }
    let mut ext = [0u8; 8];
    let raw = extension(name);
    if raw.is_empty() || raw.len() > ext.len() {
        return IconGroup::Other;
    }
    let ext = &mut ext[..raw.len()];
    ext.copy_from_slice(raw);
    ext.make_ascii_lowercase();
    match &*ext {
        b"png" | b"jpg" | b"jpeg" | b"gif" | b"webp" | b"bmp" | b"svg" | b"tif" | b"tiff"
        | b"ico" | b"heic" | b"avif" => IconGroup::Image,
        b"mp3" | b"flac" | b"ogg" | b"oga" | b"opus" | b"wav" | b"m4a" | b"aac" | b"wma" => {
            IconGroup::Audio
        }
        b"mp4" | b"mkv" | b"webm" | b"avi" | b"mov" | b"wmv" | b"m4v" | b"mpg" | b"mpeg" => {
            IconGroup::Video
        }
        b"zip" | b"tar" | b"gz" | b"tgz" | b"bz2" | b"xz" | b"zst" | b"7z" | b"rar" | b"iso" => {
            IconGroup::Archive
        }
        b"rs" | b"ts" | b"tsx" | b"js" | b"jsx" | b"mjs" | b"py" | b"c" | b"h" | b"cpp"
        | b"hpp" | b"go" | b"java" | b"kt" | b"cs" | b"rb" | b"php" | b"sh" | b"ps1" | b"lua"
        | b"css" | b"scss" | b"html" | b"json" | b"toml" | b"yaml" | b"yml" | b"xml" | b"sql" => {
            IconGroup::Code
        }
        b"txt" | b"md" | b"pdf" | b"doc" | b"docx" | b"odt" | b"rtf" | b"xls" | b"xlsx"
        | b"ods" | b"csv" | b"ppt" | b"pptx" | b"odp" | b"epub" => IconGroup::Document,
        _ => IconGroup::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_extension_but_not_a_leading_dot() {
        assert_eq!(extension(b"a.tar.gz"), b"gz");
        assert_eq!(extension(b".bashrc"), b"");
        assert_eq!(extension(b"name."), b"");
        assert_eq!(extension(b"noext"), b"");
    }

    #[test]
    fn groups_by_extension_without_regard_to_case() {
        let file = EntryKind::File;
        assert_eq!(group_for(b"a.PNG", file, None), IconGroup::Image);
        assert_eq!(group_for(b"a.Flac", file, None), IconGroup::Audio);
        assert_eq!(group_for(b"a.mkv", file, None), IconGroup::Video);
        assert_eq!(group_for(b"a.tar.ZST", file, None), IconGroup::Archive);
        assert_eq!(group_for(b"lib.rs", file, None), IconGroup::Code);
        assert_eq!(group_for(b"notes.md", file, None), IconGroup::Document);
        assert_eq!(group_for(b"a.unknown", file, None), IconGroup::Other);
        assert_eq!(
            group_for(b"a.averyveryverylongextension", file, None),
            IconGroup::Other
        );
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
    }
}
