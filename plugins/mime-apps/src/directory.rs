// The logic over a directory of applications: the handler lists, the defaults and the launches, independent of how the system answers
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashSet;
use std::io::Read;

use crate::backend::{Backend, ParentWindow};
use crate::error::{MimeAppsError, Result};
use crate::mimeapps::{desktop_id, Precedence, XdgEnv};
use crate::models::{App, Handlers, PluginStatus, TypeInfo, DIRECTORY_TYPE};
use crate::target::Target;

/// How much of a file is read when the person asks for its type to be sniffed.
const SNIFF_BYTES: u64 = 4096;

/// One application as the system describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub exec: Option<String>,
    /// The desktop file asks not to be listed (`NoDisplay`, `Hidden`).
    pub hidden: bool,
}

impl AppEntry {
    fn to_app(&self) -> App {
        App {
            id: self.id.clone(),
            name: self.name.clone(),
            icon: self.icon.clone(),
            exec_hint: self.exec.clone(),
        }
    }
}

/// What the system can be asked, with nothing else attached: gio on Linux, a fake in a test. Every answer is plain data, so no system object outlives a call.
pub trait AppDirectory: Send + Sync + 'static {
    /// The type of a file name, settled by `head` (the start of the file) when it is not empty.
    fn guess_type(&self, name: &str, head: &[u8]) -> String;
    /// A phrase and an icon name for a type.
    fn describe_type(&self, mime: &str) -> (Option<String>, Option<String>);
    /// Every application the system lists, whatever it opens: what an "Other application…" list offers.
    fn all_apps(&self) -> Vec<AppEntry>;
    /// Every application that can open the type, registered for it or not.
    fn all_for_type(&self, mime: &str) -> Vec<AppEntry>;
    /// The applications registered for the type, in the system's order.
    fn recommended_for_type(&self, mime: &str) -> Vec<AppEntry>;
    /// The applications registered for a type the given one derives from.
    fn fallback_for_type(&self, mime: &str) -> Vec<AppEntry>;
    fn default_for_type(&self, mime: &str) -> Option<AppEntry>;
    /// The application with this desktop-file id, when it is installed.
    fn app(&self, id: &str) -> Option<AppEntry>;
    /// Starts the application with these URIs.
    fn launch(&self, id: &str, uris: &[String]) -> Result<()>;
    fn set_default(&self, mime: &str, id: &str) -> Result<()>;
    fn icon_png(&self, id: &str, size: u32) -> Option<Vec<u8>>;
}

/// A [`Backend`] over an [`AppDirectory`], for every system whose applications are desktop files. It has no chooser and no settings page of its own; a platform wraps it to add them.
pub struct DirectoryBackend<D: AppDirectory> {
    dir: D,
    /// Where `mimeapps.list` is read when the directory cannot say what the default is.
    xdg: Option<XdgEnv>,
    status: Box<dyn Fn() -> PluginStatus + Send + Sync>,
}

impl<D: AppDirectory> DirectoryBackend<D> {
    pub fn new(
        dir: D,
        xdg: Option<XdgEnv>,
        status: impl Fn() -> PluginStatus + Send + Sync + 'static,
    ) -> Self {
        DirectoryBackend {
            dir,
            xdg,
            status: Box::new(status),
        }
    }

    pub fn directory(&self) -> &D {
        &self.dir
    }

    /// The type of a location.
    pub fn mime_of(&self, target: &Target, sniff: bool) -> String {
        if target.is_directory() {
            return DIRECTORY_TYPE.to_string();
        }
        let head = match (&target.path, sniff) {
            (Some(path), true) => read_head(path),
            _ => Vec::new(),
        };
        self.dir.guess_type(&target.name, &head)
    }

    fn precedence(&self) -> Option<Precedence> {
        self.xdg.as_ref().map(Precedence::load)
    }

    /// The default, the registered and the other applications of one type, hidden ones left out (except a default, which is shown so the person sees what opens the file).
    fn lists_for(&self, mime: &str) -> Lists {
        let mut recommended = self.dir.recommended_for_type(mime);
        let others = self.dir.all_for_type(mime);
        let fallback = self.dir.fallback_for_type(mime);
        let mut default = self.dir.default_for_type(mime);

        // When the directory cannot say, the `mimeapps.list` files can.
        if default.is_none() || (recommended.is_empty() && others.is_empty()) {
            if let Some(precedence) = self.precedence() {
                let installed = |id: &str| self.dir.app(id).is_some();
                let resolved = precedence.resolve(mime, &[], &installed);
                if default.is_none() {
                    default = resolved.default.as_deref().and_then(|id| self.dir.app(id));
                }
                if recommended.is_empty() && others.is_empty() {
                    recommended = resolved
                        .apps
                        .iter()
                        .filter_map(|id| self.dir.app(id))
                        .collect();
                }
            }
        }

        let mut seen: HashSet<String> = HashSet::new();
        let default = default.inspect(|entry| {
            seen.insert(desktop_id(&entry.id));
        });
        let mut take = |entries: Vec<AppEntry>| -> Vec<AppEntry> {
            entries
                .into_iter()
                .filter(|entry| !entry.hidden && seen.insert(desktop_id(&entry.id)))
                .collect()
        };
        let recommended = take(recommended);
        let mut others = take(
            others
                .into_iter()
                .chain(fallback)
                .chain(self.dir.all_apps())
                .collect(),
        );
        others.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.id.cmp(&b.id))
        });
        Lists {
            default,
            recommended,
            others,
        }
    }

    /// The application with this id, which may leave out `.desktop`; the directory is only asked with the canonical form.
    fn app(&self, id: &str) -> Option<AppEntry> {
        self.dir.app(&desktop_id(id))
    }

    fn default_app(&self, mime: &str) -> Option<AppEntry> {
        self.lists_for(mime).default
    }
}

struct Lists {
    default: Option<AppEntry>,
    recommended: Vec<AppEntry>,
    others: Vec<AppEntry>,
}

impl Lists {
    fn ids(&self) -> HashSet<String> {
        self.default
            .iter()
            .chain(&self.recommended)
            .chain(&self.others)
            .map(|entry| desktop_id(&entry.id))
            .collect()
    }
}

fn read_head(path: &std::path::Path) -> Vec<u8> {
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let mut head = Vec::new();
    let _ = file.take(SNIFF_BYTES).read_to_end(&mut head);
    head
}

impl<D: AppDirectory> Backend for DirectoryBackend<D> {
    fn status(&self) -> PluginStatus {
        let status = (self.status)();
        match &self.xdg {
            Some(xdg) => {
                let files = Precedence::load(xdg)
                    .files()
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect();
                status.with_association_files(files)
            }
            None => status,
        }
    }

    fn type_info(&self, target: &Target, sniff: bool) -> Result<TypeInfo> {
        let mime = self.mime_of(target, sniff);
        let (description, icon) = self.dir.describe_type(&mime);
        Ok(TypeInfo {
            description: description.unwrap_or_else(|| mime.clone()),
            mime,
            icon,
        })
    }

    fn handlers(&self, targets: &[Target]) -> Result<Handlers> {
        if targets.is_empty() {
            return Err(MimeAppsError::Empty);
        }
        let mut mimes: Vec<String> = Vec::new();
        for target in targets {
            let mime = self.mime_of(target, false);
            if !mimes.contains(&mime) {
                mimes.push(mime);
            }
        }
        let mut lists = mimes.iter().map(|mime| self.lists_for(mime));
        let mut first = lists.next().expect("there is at least one type");
        // With several types only the applications that open all of them are offered.
        for other in lists {
            let common = other.ids();
            let keep = |entry: &AppEntry| common.contains(&desktop_id(&entry.id));
            first.default = first.default.filter(keep);
            first.recommended.retain(keep);
            first.others.retain(keep);
        }
        Ok(Handlers {
            mime: mimes[0].clone(),
            mixed: mimes.len() > 1,
            default: first.default.as_ref().map(AppEntry::to_app),
            recommended: first.recommended.iter().map(AppEntry::to_app).collect(),
            others: first.others.iter().map(AppEntry::to_app).collect(),
        })
    }

    fn open_with(&self, targets: &[Target], app_id: &str) -> Result<()> {
        if targets.is_empty() {
            return Err(MimeAppsError::Empty);
        }
        let app = self.app(app_id).ok_or(MimeAppsError::AppNotFound)?;
        let uris: Vec<String> = targets.iter().map(|target| target.uri.clone()).collect();
        self.dir.launch(&app.id, &uris)
    }

    fn open_default(&self, targets: &[Target]) -> Result<()> {
        if targets.is_empty() {
            return Err(MimeAppsError::Empty);
        }
        // Find every application before starting any, so a location with none starts nothing.
        let mut groups: Vec<(String, Vec<String>)> = Vec::new();
        for target in targets {
            let mime = self.mime_of(target, false);
            let app = self
                .default_app(&mime)
                .ok_or(MimeAppsError::NoHandler { mime })?;
            match groups.iter_mut().find(|(id, _)| *id == app.id) {
                Some((_, uris)) => uris.push(target.uri.clone()),
                None => groups.push((app.id, vec![target.uri.clone()])),
            }
        }
        for (id, uris) in groups {
            self.dir.launch(&id, &uris)?;
        }
        Ok(())
    }

    fn choose(&self, _targets: &[Target], _parent: ParentWindow) -> Result<()> {
        Err(MimeAppsError::Unsupported)
    }

    fn set_default(&self, mime: &str, app_id: &str) -> Result<()> {
        let app = self.app(app_id).ok_or(MimeAppsError::AppNotFound)?;
        self.dir.set_default(mime, &app.id)
    }

    fn open_default_apps_settings(&self) -> Result<()> {
        Err(MimeAppsError::Unsupported)
    }

    fn knows_app(&self, app_id: &str) -> bool {
        self.app(app_id).is_some()
    }

    fn app_icon(&self, app_id: &str, size: u32) -> Option<Vec<u8>> {
        let app = self.app(app_id)?;
        self.dir.icon_png(&app.id, size)
    }
}

#[cfg(test)]
pub(crate) mod fake {
    //! A directory over a table, recording what it was asked to start. Shared with the plugin's tests.

    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use super::*;

    pub fn entry(id: &str, name: &str) -> AppEntry {
        AppEntry {
            id: id.to_string(),
            name: name.to_string(),
            icon: Some(format!("{}-icon", id.trim_end_matches(".desktop"))),
            exec: Some(format!("{} %U", id.trim_end_matches(".desktop"))),
            hidden: false,
        }
    }

    #[derive(Default)]
    pub struct Table {
        pub apps: Vec<AppEntry>,
        pub defaults: HashMap<String, String>,
        pub recommended: HashMap<String, Vec<String>>,
        pub all: HashMap<String, Vec<String>>,
        pub fallback: HashMap<String, Vec<String>>,
        /// What `all_apps` lists, by id; empty unless a test fills it.
        pub everything: Vec<String>,
        pub launched: Vec<(String, Vec<String>)>,
        pub defaults_set: Vec<(String, String)>,
    }

    #[derive(Clone, Default)]
    pub struct Fake(pub Arc<Mutex<Table>>);

    impl Fake {
        fn lookup(&self, ids: Option<&Vec<String>>) -> Vec<AppEntry> {
            let table = self.0.lock().unwrap();
            ids.into_iter()
                .flatten()
                .filter_map(|id| table.apps.iter().find(|app| app.id == *id).cloned())
                .collect()
        }
    }

    impl AppDirectory for Fake {
        fn guess_type(&self, name: &str, head: &[u8]) -> String {
            if head.starts_with(b"\x89PNG") {
                return "image/png".into();
            }
            match name
                .rsplit_once('.')
                .map(|(_, ext)| ext.to_ascii_lowercase())
            {
                Some(ext) if ext == "png" => "image/png".into(),
                Some(ext) if ext == "txt" => "text/plain".into(),
                Some(ext) if ext == "pdf" => "application/pdf".into(),
                _ => "application/octet-stream".into(),
            }
        }

        fn describe_type(&self, mime: &str) -> (Option<String>, Option<String>) {
            match mime {
                "image/png" => (Some("PNG image".into()), Some("image-x-generic".into())),
                "inode/directory" => (Some("Folder".into()), Some("folder".into())),
                _ => (None, None),
            }
        }

        fn all_apps(&self) -> Vec<AppEntry> {
            let ids = self.0.lock().unwrap().everything.clone();
            self.lookup(Some(&ids))
        }

        fn all_for_type(&self, mime: &str) -> Vec<AppEntry> {
            let all = self.0.lock().unwrap().all.get(mime).cloned();
            self.lookup(all.as_ref())
        }

        fn recommended_for_type(&self, mime: &str) -> Vec<AppEntry> {
            let list = self.0.lock().unwrap().recommended.get(mime).cloned();
            self.lookup(list.as_ref())
        }

        fn fallback_for_type(&self, mime: &str) -> Vec<AppEntry> {
            let list = self.0.lock().unwrap().fallback.get(mime).cloned();
            self.lookup(list.as_ref())
        }

        fn default_for_type(&self, mime: &str) -> Option<AppEntry> {
            let id = self.0.lock().unwrap().defaults.get(mime).cloned()?;
            self.app(&id)
        }

        fn app(&self, id: &str) -> Option<AppEntry> {
            let id = desktop_id(id);
            self.0
                .lock()
                .unwrap()
                .apps
                .iter()
                .find(|app| app.id == id)
                .cloned()
        }

        fn launch(&self, id: &str, uris: &[String]) -> Result<()> {
            self.0
                .lock()
                .unwrap()
                .launched
                .push((id.to_string(), uris.to_vec()));
            Ok(())
        }

        fn set_default(&self, mime: &str, id: &str) -> Result<()> {
            self.0
                .lock()
                .unwrap()
                .defaults_set
                .push((mime.to_string(), id.to_string()));
            Ok(())
        }

        fn icon_png(&self, id: &str, size: u32) -> Option<Vec<u8>> {
            Some(format!("png:{id}:{size}").into_bytes())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::*;
    use super::*;
    use crate::models::{Flavour, Reason};
    use crate::target::parse;

    fn status() -> PluginStatus {
        PluginStatus::all_unavailable(Flavour::Gio, Reason::NoSystemChooser, "test")
    }

    fn backend(table: Fake, xdg: Option<XdgEnv>) -> DirectoryBackend<Fake> {
        DirectoryBackend::new(table, xdg, status)
    }

    fn images() -> Fake {
        let fake = Fake::default();
        {
            let mut table = fake.0.lock().unwrap();
            let mut hidden = entry("helper.desktop", "Helper");
            hidden.hidden = true;
            table.apps = vec![
                entry("eog.desktop", "Image Viewer"),
                entry("gimp.desktop", "GIMP"),
                entry("krita.desktop", "Krita"),
                entry("zed.desktop", "Zed"),
                hidden,
            ];
            table
                .defaults
                .insert("image/png".into(), "eog.desktop".into());
            table.recommended.insert(
                "image/png".into(),
                vec![
                    "eog.desktop".into(),
                    "gimp.desktop".into(),
                    "helper.desktop".into(),
                ],
            );
            table.all.insert(
                "image/png".into(),
                vec![
                    "zed.desktop".into(),
                    "gimp.desktop".into(),
                    "krita.desktop".into(),
                    "eog.desktop".into(),
                ],
            );
            table
                .fallback
                .insert("image/png".into(), vec!["krita.desktop".into()]);
        }
        fake
    }

    fn ids(apps: &[App]) -> Vec<&str> {
        apps.iter().map(|app| app.id.as_str()).collect()
    }

    #[test]
    fn handlers_put_the_default_first_then_recommended_then_the_others_by_name() {
        let backend = backend(images(), None);
        let handlers = backend.handlers(&[parse("/p/cat.png").unwrap()]).unwrap();
        assert_eq!(handlers.mime, "image/png");
        assert!(!handlers.mixed);
        assert_eq!(handlers.default.unwrap().id, "eog.desktop");
        // The default is not repeated, the hidden app is dropped.
        assert_eq!(ids(&handlers.recommended), ["gimp.desktop"]);
        // Duplicates (`krita` is in `all` and the fallback) collapse; sorted by name.
        assert_eq!(ids(&handlers.others), ["krita.desktop", "zed.desktop"]);
    }

    #[test]
    fn every_other_installed_application_is_offered_after_the_registered_ones() {
        let fake = images();
        fake.0.lock().unwrap().everything = vec![
            "zed.desktop".into(),
            "eog.desktop".into(),
            "helper.desktop".into(),
            "kate.desktop".into(),
        ];
        fake.0
            .lock()
            .unwrap()
            .apps
            .push(entry("kate.desktop", "Kate"));
        let handlers = backend(fake, None)
            .handlers(&[parse("/p/cat.png").unwrap()])
            .unwrap();
        assert_eq!(
            ids(&handlers.others),
            ["kate.desktop", "krita.desktop", "zed.desktop"]
        );
    }

    #[test]
    fn an_app_carries_its_icon_name_and_command_hint() {
        let backend = backend(images(), None);
        let handlers = backend.handlers(&[parse("/p/cat.png").unwrap()]).unwrap();
        let default = handlers.default.unwrap();
        assert_eq!(default.name, "Image Viewer");
        assert_eq!(default.icon.as_deref(), Some("eog-icon"));
        assert_eq!(default.exec_hint.as_deref(), Some("eog %U"));
    }

    #[test]
    fn a_hidden_default_is_still_shown_as_the_default() {
        let fake = images();
        fake.0
            .lock()
            .unwrap()
            .defaults
            .insert("image/png".into(), "helper.desktop".into());
        let handlers = backend(fake, None)
            .handlers(&[parse("/p/cat.png").unwrap()])
            .unwrap();
        assert_eq!(handlers.default.unwrap().id, "helper.desktop");
        assert!(!ids(&handlers.recommended).contains(&"helper.desktop"));
    }

    #[test]
    fn a_directory_has_the_directory_type() {
        let tmp = tempfile::tempdir().unwrap();
        let backend = backend(images(), None);
        let target = parse(&tmp.path().display().to_string()).unwrap();
        let info = backend.type_info(&target, false).unwrap();
        assert_eq!(info.mime, "inode/directory");
        assert_eq!(info.description, "Folder");
        assert_eq!(info.icon.as_deref(), Some("folder"));
        let remote = parse("sftp://host/home/").unwrap();
        assert_eq!(
            backend.type_info(&remote, false).unwrap().mime,
            "inode/directory"
        );
    }

    #[test]
    fn the_type_comes_from_the_name_and_sniffing_only_on_request() {
        let tmp = tempfile::tempdir().unwrap();
        let disguised = tmp.path().join("picture.dat");
        std::fs::write(&disguised, b"\x89PNG\r\n\x1a\nrest").unwrap();
        let target = parse(&disguised.display().to_string()).unwrap();
        let backend = backend(images(), None);
        assert_eq!(
            backend.type_info(&target, false).unwrap().mime,
            "application/octet-stream"
        );
        assert_eq!(backend.type_info(&target, true).unwrap().mime, "image/png");
        // A description falls back to the type itself.
        assert_eq!(
            backend
                .type_info(&parse("/p/a.pdf").unwrap(), false)
                .unwrap()
                .description,
            "application/pdf"
        );
    }

    #[test]
    fn several_types_offer_only_the_applications_that_open_all_of_them() {
        let fake = images();
        {
            let mut table = fake.0.lock().unwrap();
            table.recommended.insert(
                "text/plain".into(),
                vec!["gimp.desktop".into(), "zed.desktop".into()],
            );
            table
                .defaults
                .insert("text/plain".into(), "zed.desktop".into());
        }
        let handlers = backend(fake, None)
            .handlers(&[parse("/p/a.png").unwrap(), parse("/p/b.txt").unwrap()])
            .unwrap();
        assert!(handlers.mixed);
        assert_eq!(handlers.mime, "image/png");
        // `eog` is the default for images but does not open text.
        assert_eq!(handlers.default, None);
        assert_eq!(ids(&handlers.recommended), ["gimp.desktop"]);
        assert_eq!(ids(&handlers.others), ["zed.desktop"]);
    }

    #[test]
    fn open_default_starts_each_application_once_with_its_locations() {
        let fake = images();
        fake.0
            .lock()
            .unwrap()
            .defaults
            .insert("text/plain".into(), "zed.desktop".into());
        let backend = backend(fake.clone(), None);
        backend
            .open_default(&[
                parse("/p/a.png").unwrap(),
                parse("/p/b.txt").unwrap(),
                parse("/p/c.png").unwrap(),
            ])
            .unwrap();
        let launched = fake.0.lock().unwrap().launched.clone();
        assert_eq!(
            launched,
            [
                (
                    "eog.desktop".to_string(),
                    vec!["file:///p/a.png".to_string(), "file:///p/c.png".to_string()]
                ),
                (
                    "zed.desktop".to_string(),
                    vec!["file:///p/b.txt".to_string()]
                ),
            ]
        );
    }

    #[test]
    fn open_default_starts_nothing_when_one_location_has_no_handler() {
        let fake = images();
        let backend = backend(fake.clone(), None);
        let error = backend
            .open_default(&[parse("/p/a.png").unwrap(), parse("/p/x.pdf").unwrap()])
            .unwrap_err();
        assert_eq!(
            error,
            MimeAppsError::NoHandler {
                mime: "application/pdf".into()
            }
        );
        assert!(fake.0.lock().unwrap().launched.is_empty());
    }

    #[test]
    fn open_with_accepts_an_id_with_or_without_the_suffix_and_rejects_an_unknown_one() {
        let fake = images();
        let backend = backend(fake.clone(), None);
        backend
            .open_with(&[parse("/p/a.png").unwrap()], "gimp")
            .unwrap();
        assert_eq!(fake.0.lock().unwrap().launched[0].0, "gimp.desktop");
        assert_eq!(
            backend.open_with(&[parse("/p/a.png").unwrap()], "nope.desktop"),
            Err(MimeAppsError::AppNotFound)
        );
        assert_eq!(backend.open_with(&[], "gimp"), Err(MimeAppsError::Empty));
        assert_eq!(fake.0.lock().unwrap().launched.len(), 1);
    }

    #[test]
    fn set_default_checks_the_application_and_passes_the_canonical_id() {
        let fake = images();
        let backend = backend(fake.clone(), None);
        backend.set_default("image/png", "gimp").unwrap();
        assert_eq!(
            fake.0.lock().unwrap().defaults_set,
            [("image/png".to_string(), "gimp.desktop".to_string())]
        );
        assert_eq!(
            backend.set_default("image/png", "nope"),
            Err(MimeAppsError::AppNotFound)
        );
    }

    #[test]
    fn mimeapps_list_answers_when_the_directory_cannot() {
        let tmp = tempfile::tempdir().unwrap();
        let config = tmp.path().join("config");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::write(
            config.join("mimeapps.list"),
            "[Default Applications]\nimage/png=krita\n[Added Associations]\nimage/png=zed.desktop;gone.desktop\n",
        )
        .unwrap();
        let xdg = XdgEnv {
            config_home: config.clone(),
            config_dirs: Vec::new(),
            data_home: tmp.path().join("share"),
            data_dirs: Vec::new(),
            desktops: Vec::new(),
        };
        let fake = images();
        {
            let mut table = fake.0.lock().unwrap();
            table.defaults.clear();
            table.recommended.clear();
            table.all.clear();
            table.fallback.clear();
        }
        let backend = backend(fake, Some(xdg));
        let handlers = backend.handlers(&[parse("/p/a.png").unwrap()]).unwrap();
        assert_eq!(handlers.default.unwrap().id, "krita.desktop");
        assert_eq!(ids(&handlers.recommended), ["zed.desktop"]);
        let status = backend.status();
        assert_eq!(
            status.association_files,
            [config.join("mimeapps.list").display().to_string()]
        );
    }

    #[test]
    fn icons_are_served_only_for_installed_applications() {
        let backend = backend(images(), None);
        assert!(backend.knows_app("eog.desktop"));
        assert_eq!(
            backend.app_icon("eog.desktop", 32),
            Some(b"png:eog.desktop:32".to_vec())
        );
        assert!(!backend.knows_app("../../etc/passwd"));
        assert_eq!(backend.app_icon("../../etc/passwd", 32), None);
    }

    #[test]
    fn there_is_no_chooser_or_settings_page_without_a_platform_wrapper() {
        let backend = backend(images(), None);
        assert_eq!(backend.choose(&[], None), Err(MimeAppsError::Unsupported));
        assert_eq!(
            backend.open_default_apps_settings(),
            Err(MimeAppsError::Unsupported)
        );
    }
}
