// The seam between the plugin's logic and the operating system: what a platform backend has to provide
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::error::Result;
use crate::models::{Handlers, PluginStatus, TypeInfo};
use crate::target::Target;
use crate::typeicons::IconRequest;

/// The native handle of the window a system dialog should belong to (an `HWND` on Windows), when there is one.
pub type ParentWindow = Option<isize>;

/// A platform's types and applications. The real ones are `linux.rs` and `windows.rs`; tests inject a fake. Every method is blocking: the plugin calls them off the async runtime, so an implementation may take as long as the system does.
///
/// A backend never starts anything that was not asked for, and never changes a default except in `set_default`.
pub trait Backend: Send + Sync + 'static {
    /// What works on this system. Probing may touch the file system, so this is asked when a status is wanted rather than once.
    fn status(&self) -> PluginStatus;

    /// The type of one location. `sniff` allows reading the start of a local file to settle what its name does not.
    fn type_info(&self, target: &Target, sniff: bool) -> Result<TypeInfo>;

    /// The applications for the locations' type, or the ones that open every type when they differ.
    fn handlers(&self, targets: &[Target]) -> Result<Handlers>;

    /// Opens the locations in the application with this id.
    fn open_with(&self, targets: &[Target], app_id: &str) -> Result<()>;

    /// Opens each location in its default application, one start per application.
    fn open_default(&self, targets: &[Target]) -> Result<()>;

    /// Asks the system to let the person choose an application and opens the locations in it. `Unsupported` when the system has no chooser of its own.
    fn choose(&self, targets: &[Target], parent: ParentWindow) -> Result<()>;

    /// Makes the application the default for the type.
    fn set_default(&self, mime: &str, app_id: &str) -> Result<()>;

    /// Opens the system's own page for choosing default applications. `Unsupported` where there is none.
    fn open_default_apps_settings(&self) -> Result<()>;

    /// Whether an application with this id exists; the `appicon://` scheme serves nothing else.
    fn knows_app(&self, app_id: &str) -> bool;

    /// The application's icon as PNG bytes at about `size` pixels. May need the main thread; the scheme handler runs there.
    fn app_icon(&self, app_id: &str, size: u32) -> Option<Vec<u8>>;

    /// The icon theme the system draws with now, for the cache key of a request that names none. `None` where there is no such thing.
    fn type_icon_theme(&self) -> Option<String> {
        None
    }

    /// The picture of an icon for a type or a folder as PNG bytes, `size * scale` pixels along an edge. May need the main thread; the scheme handler runs there.
    fn type_icon(&self, _request: &IconRequest) -> Option<Vec<u8>> {
        None
    }

    /// Forgets what the platform cached about icons (theme files read, shell icon lists): the theme changed or its files did.
    fn refresh_type_icons(&self) {}
}
