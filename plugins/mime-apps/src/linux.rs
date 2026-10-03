// The Linux backend: gio's `AppInfo` over `mimeapps.list` and the desktop files, and the OpenURI portal inside a Flatpak sandbox
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Every gio call is behind [`AppDirectory`], so the logic over it is the same as under a test. gio objects are not `Send`; each call makes the ones it needs, copies what it wants into plain data and drops them.
//!
//! Starting an application goes through the main thread when the plugin has an app handle, because only the toolkit's launch context knows the display's activation token (so the application's window is raised and focused on Wayland). Icons are drawn through GTK's icon theme and so also need the main thread, which is where the `appicon://` scheme handler runs.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use gio::glib::translate::FromGlibPtrFull;
use gio::prelude::*;
use gtk::prelude::{GtkSettingsExt, IconThemeExt};
use tauri::{AppHandle, Runtime};

use crate::backend::{Backend, ParentWindow};
use crate::directory::{AppDirectory, AppEntry, DirectoryBackend};
use crate::error::{MimeAppsError, Result};
use crate::mimeapps::XdgEnv;
use crate::models::{Handlers, PluginStatus, TypeInfo};
use crate::status::{linux_status, LinuxEnv};
use crate::target::Target;
use crate::typeicons::{candidate_names, IconRequest, NameSource};

/// How long a launch waits for the main thread before giving up.
const MAIN_THREAD_WAIT: Duration = Duration::from_secs(10);

/// Runs a job on the main thread.
pub type MainRunner = Arc<dyn Fn(Box<dyn FnOnce() + Send>) -> bool + Send + Sync>;

/// gio, as an [`AppDirectory`].
pub struct GioDirectory {
    main: Option<MainRunner>,
}

impl Default for GioDirectory {
    fn default() -> Self {
        Self::new()
    }
}

impl GioDirectory {
    /// A directory that starts applications with a plain launch context. Used where there is no event loop, such as the ignored live test.
    pub fn new() -> Self {
        GioDirectory { main: None }
    }

    pub fn with_main_thread(main: MainRunner) -> Self {
        GioDirectory { main: Some(main) }
    }
}

/// The type of a name alone. gio's binding passes an empty slice as "the file has no content", which a guess reads as `application/x-zerosize`; asking with no data at all is what means "going by the name".
fn guess_by_name(name: &str) -> String {
    let Ok(name) = std::ffi::CString::new(name) else {
        return "application/octet-stream".into();
    };
    let mut uncertain = gio::glib::ffi::GFALSE;
    // SAFETY: `name` is a valid C string for the call; with no data and a size of 0 the function guesses from the name; the result is a newly allocated string that `from_glib_full` takes over.
    unsafe {
        let guess =
            gio::ffi::g_content_type_guess(name.as_ptr(), std::ptr::null(), 0, &mut uncertain);
        gio::glib::GString::from_glib_full(guess).to_string()
    }
}

fn entry_of(info: &gio::AppInfo) -> Option<AppEntry> {
    let id = info.id()?.to_string();
    Some(AppEntry {
        id,
        name: info.name().to_string(),
        icon: info.icon().and_then(|icon| icon_name(&icon)),
        exec: info
            .commandline()
            .map(|path| path.display().to_string())
            .filter(|line| !line.is_empty()),
        hidden: !info.should_show(),
    })
}

/// The first name of a themed icon, or the path of a file icon.
fn icon_name(icon: &gio::Icon) -> Option<String> {
    if let Some(themed) = icon.downcast_ref::<gio::ThemedIcon>() {
        return themed.names().first().map(|name| name.to_string());
    }
    icon.downcast_ref::<gio::FileIcon>()
        .and_then(|file| file.file().path())
        .map(|path| path.display().to_string())
}

fn entries(infos: Vec<gio::AppInfo>) -> Vec<AppEntry> {
    infos.iter().filter_map(entry_of).collect()
}

fn desktop_app(id: &str) -> Option<gio::AppInfo> {
    let id = crate::mimeapps::desktop_id(id);
    gio::DesktopAppInfo::new(&id).map(|info| info.upcast())
}

impl AppDirectory for GioDirectory {
    fn guess_type(&self, name: &str, head: &[u8]) -> String {
        if head.is_empty() {
            return guess_by_name(name);
        }
        let (content_type, _uncertain) = gio::content_type_guess(Some(name), head);
        content_type.to_string()
    }

    fn describe_type(&self, mime: &str) -> (Option<String>, Option<String>) {
        let description = gio::content_type_get_description(mime).to_string();
        let icon = icon_name(&gio::content_type_get_icon(mime));
        (Some(description).filter(|text| !text.is_empty()), icon)
    }

    fn all_apps(&self) -> Vec<AppEntry> {
        entries(gio::AppInfo::all())
    }

    fn all_for_type(&self, mime: &str) -> Vec<AppEntry> {
        entries(gio::AppInfo::all_for_type(mime))
    }

    fn recommended_for_type(&self, mime: &str) -> Vec<AppEntry> {
        entries(gio::AppInfo::recommended_for_type(mime))
    }

    fn fallback_for_type(&self, mime: &str) -> Vec<AppEntry> {
        entries(gio::AppInfo::fallback_for_type(mime))
    }

    fn default_for_type(&self, mime: &str) -> Option<AppEntry> {
        gio::AppInfo::default_for_type(mime, false).and_then(|info| entry_of(&info))
    }

    fn app(&self, id: &str) -> Option<AppEntry> {
        desktop_app(id).and_then(|info| entry_of(&info))
    }

    fn launch(&self, id: &str, uris: &[String]) -> Result<()> {
        let id = id.to_string();
        let uris = uris.to_vec();
        let job = move || -> Result<()> {
            let info = desktop_app(&id).ok_or(MimeAppsError::AppNotFound)?;
            let refs: Vec<&str> = uris.iter().map(String::as_str).collect();
            // The toolkit's context carries the activation token; without a display, a plain one.
            let context = gtk::gdk::Display::default()
                .filter(|_| gtk::is_initialized_main_thread())
                .and_then(|display| display.app_launch_context());
            let result = match &context {
                Some(context) => info.launch_uris(&refs, Some(context)),
                None => info.launch_uris(&refs, None::<&gio::AppLaunchContext>),
            };
            result.map_err(|error| MimeAppsError::failed(error.message()))
        };
        match &self.main {
            Some(run) => {
                let (tx, rx) = mpsc::channel();
                let sent = run(Box::new(move || {
                    let _ = tx.send(job());
                }));
                if !sent {
                    return Err(MimeAppsError::failed("the main thread is not running"));
                }
                rx.recv_timeout(MAIN_THREAD_WAIT)
                    .map_err(|_| MimeAppsError::failed("the main thread did not answer"))?
            }
            None => job(),
        }
    }

    fn set_default(&self, mime: &str, id: &str) -> Result<()> {
        let info = desktop_app(id).ok_or(MimeAppsError::AppNotFound)?;
        info.set_as_default_for_type(mime)
            .map_err(|error| MimeAppsError::failed(error.message()))
    }

    fn icon_png(&self, id: &str, size: u32) -> Option<Vec<u8>> {
        // GTK may only be used from the thread that initialised it.
        if !gtk::is_initialized_main_thread() {
            return None;
        }
        let info = desktop_app(id)?;
        let icon = info.icon()?;
        let size = i32::try_from(size).ok()?;
        let pixbuf = if let Some(file) = icon.downcast_ref::<gio::FileIcon>() {
            let path = file.file().path()?;
            gtk::gdk_pixbuf::Pixbuf::from_file_at_size(path, size, size).ok()?
        } else {
            let theme = gtk::IconTheme::default()?;
            let found = theme.lookup_by_gicon(&icon, size, gtk::IconLookupFlags::FORCE_SIZE)?;
            found.load_icon().ok()?
        };
        pixbuf.save_to_bufferv("png", &[]).ok()
    }
}

/// What gio says about content types, as the names a theme is asked for.
struct GioNames;

impl NameSource for GioNames {
    fn mime_names(&self, mime: &str) -> Vec<String> {
        icon_names(&gio::content_type_get_icon(mime))
    }

    fn mime_for_extension(&self, extension: &str) -> String {
        guess_by_name(&format!("x.{extension}"))
    }
}

/// Every name of a themed icon, in the order gio lists them.
fn icon_names(icon: &gio::Icon) -> Vec<String> {
    icon.downcast_ref::<gio::ThemedIcon>()
        .map(|themed| themed.names().iter().map(|name| name.to_string()).collect())
        .unwrap_or_default()
}

/// How many icon themes one thread keeps open besides the default one.
const OPEN_THEMES: usize = 8;

thread_local! {
    /// The themes that were asked for by name, as GTK's icon theme objects. GTK objects live on the main thread, which is the only thread that draws icons, so this is plain thread-local state.
    static THEMES: RefCell<HashMap<String, gtk::IconTheme>> = RefCell::new(HashMap::new());
}

/// The icon theme the system is set to, as GTK reads it from the settings daemon.
fn live_theme_name() -> Option<String> {
    gtk::Settings::default()?
        .gtk_icon_theme_name()
        .map(|name| name.to_string())
        .filter(|name| !name.is_empty())
}

/// Runs `job` with the theme named, or the default one when none is named or the name is the one the system is set to now (so it follows the settings and their inheritance exactly).
fn with_theme<T>(name: Option<&str>, job: impl FnOnce(&gtk::IconTheme) -> Option<T>) -> Option<T> {
    let live = live_theme_name();
    match name.filter(|name| Some(*name) != live.as_deref()) {
        None => job(&gtk::IconTheme::default()?),
        Some(name) => THEMES.with(|themes| {
            let mut themes = themes.borrow_mut();
            if !themes.contains_key(name) {
                if themes.len() >= OPEN_THEMES {
                    themes.clear();
                }
                let theme = gtk::IconTheme::new();
                theme.set_custom_theme(Some(name));
                themes.insert(name.to_string(), theme);
            }
            job(&themes[name])
        }),
    }
}

/// The icons of the user's GTK icon theme, drawn the way a GTK file manager draws them: the names gio gives a type, looked up in the theme at the size and scale asked for. Symbolic icons are never used.
#[derive(Default)]
pub struct GtkIcons {
    /// The theme's files may have changed: the next draw on the main thread rescans.
    stale: AtomicBool,
}

impl GtkIcons {
    pub fn theme_name(&self) -> Option<String> {
        if !gtk::is_initialized_main_thread() {
            return None;
        }
        live_theme_name()
    }

    pub fn invalidate(&self) {
        self.stale.store(true, Ordering::SeqCst);
    }

    /// The icon as PNG bytes, or `None` when GTK is not running here or the theme has none of the names. Main thread only.
    pub fn render(&self, request: &IconRequest) -> Option<Vec<u8>> {
        if !gtk::is_initialized_main_thread() {
            return None;
        }
        if self.stale.swap(false, Ordering::SeqCst) {
            THEMES.with(|themes| themes.borrow_mut().clear());
            if let Some(theme) = gtk::IconTheme::default() {
                theme.rescan_if_needed();
            }
        }
        let names = candidate_names(&request.kind, &GioNames);
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        if refs.is_empty() {
            return None;
        }
        let size = i32::try_from(request.size).ok()?;
        let scale = i32::try_from(request.scale).ok()?;
        with_theme(request.theme.as_deref(), |theme| {
            // GTK's own "choose" takes the names as one list; looked up one by one, in order and with no generic fallback, a name earlier in the list wins over any later one.
            let flags = gtk::IconLookupFlags::FORCE_SIZE | gtk::IconLookupFlags::FORCE_REGULAR;
            let info = refs
                .iter()
                .find_map(|name| theme.lookup_icon_for_scale(name, size, scale, flags))?;
            info.load_icon().ok()?.save_to_bufferv("png", &[]).ok()
        })
    }
}

/// The Linux backend: gio, or the portal in a sandbox.
pub struct Platform {
    inner: DirectoryBackend<GioDirectory>,
    env: LinuxEnv,
    icons: GtkIcons,
}

impl Default for Platform {
    fn default() -> Self {
        Self::new()
    }
}

impl Platform {
    /// Over the real system, with no event loop to hand launches to.
    pub fn new() -> Self {
        Platform::with(GioDirectory::new(), LinuxEnv::detect(), XdgEnv::from_env())
    }

    /// Over the real system and the app's main thread.
    pub fn for_app<R: Runtime>(app: &AppHandle<R>) -> Self {
        let handle = app.clone();
        let runner: MainRunner = Arc::new(move |job| handle.run_on_main_thread(job).is_ok());
        Platform::with(
            GioDirectory::with_main_thread(runner),
            LinuxEnv::detect(),
            XdgEnv::from_env(),
        )
    }

    pub fn with(dir: GioDirectory, env: LinuxEnv, xdg: XdgEnv) -> Self {
        Platform {
            inner: DirectoryBackend::new(dir, Some(xdg), move || linux_status(env)),
            env,
            icons: GtkIcons::default(),
        }
    }
}

impl Backend for Platform {
    fn status(&self) -> PluginStatus {
        self.inner.status()
    }

    fn type_info(&self, target: &Target, sniff: bool) -> Result<TypeInfo> {
        self.inner.type_info(target, sniff)
    }

    fn handlers(&self, targets: &[Target]) -> Result<Handlers> {
        if self.env.flatpak {
            return Err(MimeAppsError::Unsupported);
        }
        self.inner.handlers(targets)
    }

    fn open_with(&self, targets: &[Target], app_id: &str) -> Result<()> {
        if self.env.flatpak {
            return Err(MimeAppsError::Unsupported);
        }
        self.inner.open_with(targets, app_id)
    }

    fn open_default(&self, targets: &[Target]) -> Result<()> {
        if self.env.flatpak {
            return portal::open(targets, false);
        }
        self.inner.open_default(targets)
    }

    fn choose(&self, targets: &[Target], _parent: ParentWindow) -> Result<()> {
        if self.env.flatpak {
            return portal::open(targets, true);
        }
        Err(MimeAppsError::Unsupported)
    }

    fn set_default(&self, mime: &str, app_id: &str) -> Result<()> {
        if self.env.flatpak {
            return Err(MimeAppsError::Unsupported);
        }
        self.inner.set_default(mime, app_id)
    }

    fn open_default_apps_settings(&self) -> Result<()> {
        Err(MimeAppsError::Unsupported)
    }

    fn knows_app(&self, app_id: &str) -> bool {
        !self.env.flatpak && self.inner.knows_app(app_id)
    }

    fn app_icon(&self, app_id: &str, size: u32) -> Option<Vec<u8>> {
        if self.env.flatpak {
            return None;
        }
        self.inner.app_icon(app_id, size)
    }

    fn type_icon_theme(&self) -> Option<String> {
        self.icons.theme_name()
    }

    fn type_icon(&self, request: &IconRequest) -> Option<Vec<u8>> {
        self.icons.render(request)
    }

    fn refresh_type_icons(&self) {
        self.icons.invalidate();
    }
}

/// The OpenURI portal, which a Flatpak sandbox uses to open a file in an application on the host.
mod portal {
    use std::fs::File;

    use ashpd::desktop::open_uri::{OpenDirectoryRequest, OpenFileRequest};
    use ashpd::desktop::ResponseError;

    use super::*;

    fn map(error: ashpd::Error) -> MimeAppsError {
        match error {
            ashpd::Error::Response(ResponseError::Cancelled) => MimeAppsError::Cancelled,
            other => MimeAppsError::failed(other.to_string()),
        }
    }

    /// Opens each location through the portal. With `ask` the portal always shows its chooser; without it the portal opens the default application, or asks only when there is none.
    pub fn open(targets: &[Target], ask: bool) -> Result<()> {
        if targets.is_empty() {
            return Err(MimeAppsError::Empty);
        }
        tauri::async_runtime::block_on(async {
            for target in targets {
                match &target.path {
                    Some(path) if target.is_directory() && !ask => {
                        let directory = File::open(path)
                            .map_err(|error| MimeAppsError::failed(error.to_string()))?;
                        OpenDirectoryRequest::default()
                            .send(&directory)
                            .await
                            .map_err(map)?
                            .response()
                            .map_err(map)?;
                    }
                    Some(path) => {
                        let file = File::open(path)
                            .map_err(|error| MimeAppsError::failed(error.to_string()))?;
                        OpenFileRequest::default()
                            .ask(ask)
                            .send_file(&file)
                            .await
                            .map_err(map)?
                            .response()
                            .map_err(map)?;
                    }
                    None => {
                        let uri = ashpd::Uri::parse(&target.uri).map_err(|_| {
                            MimeAppsError::InvalidUri {
                                uri: target.uri.clone(),
                            }
                        })?;
                        OpenFileRequest::default()
                            .ask(ask)
                            .send_uri(&uri)
                            .await
                            .map_err(map)?
                            .response()
                            .map_err(map)?;
                    }
                }
            }
            Ok(())
        })
    }
}
