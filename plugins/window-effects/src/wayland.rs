// Blurs behind a window on Wayland, through `ext_background_effect_manager_v1` or KDE's `org_kde_kwin_blur_manager`, on GTK's own connection and surface
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! GTK3 owns the Wayland connection and the window's `wl_surface`, so the plugin shares the
//! connection (see [`crate::ffi`]) and puts a blur object of its own on GTK's surface. It keeps one
//! event queue and its objects for the life of the process, because libwayland objects must not
//! outlive the queue they dispatch on, and keeps each window's blur object so that it can be
//! replaced or removed (a surface may have only one).
//!
//! Everything here must run on the thread that runs GTK's main loop.

use std::collections::HashMap;

use wayland_client::globals::{registry_queue_init, GlobalList, GlobalListContents};
use wayland_client::protocol::{
    wl_compositor::WlCompositor, wl_region::WlRegion, wl_registry::WlRegistry,
    wl_surface::WlSurface,
};
use wayland_client::{delegate_noop, Connection, Dispatch, EventQueue, QueueHandle, WEnum};
use wayland_protocols::ext::background_effect::v1::client::{
    ext_background_effect_manager_v1::{self, Capability, ExtBackgroundEffectManagerV1},
    ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1,
};
use wayland_protocols_plasma::blur::client::{
    org_kde_kwin_blur::OrgKdeKwinBlur, org_kde_kwin_blur_manager::OrgKdeKwinBlurManager,
};

use crate::ffi::{self, RawProxy};
use crate::models::Rect;
use crate::status::{BlurPath, EXT_BACKGROUND_EFFECT};

/// Stands for the whole window when a protocol wants a region: the compositor clips it to the surface.
const WHOLE_SURFACE: i32 = 32767;

#[derive(Default)]
struct State {
    /// The standard protocol's manager says the compositor can blur.
    ext_blur: bool,
}

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as wayland_client::Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtBackgroundEffectManagerV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ExtBackgroundEffectManagerV1,
        event: ext_background_effect_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_background_effect_manager_v1::Event::Capabilities { flags } = event {
            state.ext_blur = match flags {
                WEnum::Value(flags) => flags.contains(Capability::Blur),
                WEnum::Unknown(bits) => bits & Capability::Blur.bits() != 0,
            };
        }
    }
}

delegate_noop!(State: ignore WlCompositor);
delegate_noop!(State: ignore WlRegion);
delegate_noop!(State: ignore ExtBackgroundEffectSurfaceV1);
delegate_noop!(State: ignore OrgKdeKwinBlurManager);
delegate_noop!(State: ignore OrgKdeKwinBlur);

/// What this plugin put on one window's surface.
enum Applied {
    Ext(ExtBackgroundEffectSurfaceV1),
    Kde(OrgKdeKwinBlur),
}

/// The plugin's share of GTK's Wayland connection.
pub struct Session {
    conn: Connection,
    queue: EventQueue<State>,
    state: State,
    globals: GlobalList,
    compositor: Option<WlCompositor>,
    ext: Option<ExtBackgroundEffectManagerV1>,
    kde: Option<OrgKdeKwinBlurManager>,
    applied: HashMap<String, Applied>,
}

impl Session {
    /// Connects over GDK's display and binds the managers the compositor offers.
    ///
    /// # Safety
    /// `display` must be GDK's live `wl_display`, and this must run on the GTK main thread.
    pub unsafe fn new(display: RawProxy) -> Result<Self, String> {
        // SAFETY: the caller hands over GDK's live display (see `ffi`).
        let conn = unsafe { ffi::shared_connection(display) };
        let (globals, queue) =
            registry_queue_init::<State>(&conn).map_err(|error| error.to_string())?;
        let qh = queue.handle();
        let compositor = globals.bind::<WlCompositor, _, _>(&qh, 1..=1, ()).ok();
        let ext = globals
            .bind::<ExtBackgroundEffectManagerV1, _, _>(&qh, 1..=1, ())
            .ok();
        let kde = globals
            .bind::<OrgKdeKwinBlurManager, _, _>(&qh, 1..=1, ())
            .ok();
        let mut session = Session {
            conn,
            queue,
            state: State::default(),
            globals,
            compositor,
            ext,
            kde,
            applied: HashMap::new(),
        };
        // The manager's capabilities arrive after the bind.
        session.sync()?;
        Ok(session)
    }

    fn sync(&mut self) -> Result<(), String> {
        self.queue
            .roundtrip(&mut self.state)
            .map(drop)
            .map_err(|error| error.to_string())
    }

    /// The interfaces the compositor offers, with the standard blur manager left out when it said it cannot blur.
    pub fn globals(&mut self) -> Result<Vec<String>, String> {
        self.sync()?;
        let mut names: Vec<String> = self
            .globals
            .contents()
            .clone_list()
            .into_iter()
            .map(|global| global.interface)
            .collect();
        if !self.state.ext_blur {
            names.retain(|name| name != EXT_BACKGROUND_EFFECT);
        }
        Ok(names)
    }

    fn region(&self, rects: Option<&[Rect]>) -> Result<WlRegion, String> {
        let compositor = self
            .compositor
            .as_ref()
            .ok_or("the compositor has no wl_compositor")?;
        let region = compositor.create_region(&self.queue.handle(), ());
        match rects {
            Some(rects) => {
                for rect in rects {
                    region.add(rect.x, rect.y, rect.width, rect.height);
                }
            }
            None => region.add(0, 0, WHOLE_SURFACE, WHOLE_SURFACE),
        }
        Ok(region)
    }

    /// Puts a blur behind `surface` over `rects` (the whole surface when `None`), replacing what this session put there under `label`.
    ///
    /// # Safety
    /// `surface` must be the live `wl_surface` of the window `label` names.
    pub unsafe fn blur(
        &mut self,
        label: &str,
        surface: RawProxy,
        path: BlurPath,
        rects: Option<&[Rect]>,
    ) -> Result<(), String> {
        // SAFETY: the caller vouches for the surface; wayland-rs checks its interface.
        let surface: WlSurface = unsafe { ffi::foreign(&self.conn, surface) }
            .ok_or("the window's wl_surface could not be reached")?;
        self.remove(label, Some(&surface));
        let qh = self.queue.handle();
        let applied = match path {
            BlurPath::ExtBackgroundEffect => {
                let manager = self
                    .ext
                    .as_ref()
                    .ok_or("the compositor has no ext_background_effect_manager_v1")?;
                let object = manager.get_background_effect(&surface, &qh, ());
                let region = self.region(rects)?;
                object.set_blur_region(Some(&region));
                region.destroy();
                Applied::Ext(object)
            }
            BlurPath::KdeBlur => {
                let manager = self
                    .kde
                    .as_ref()
                    .ok_or("the compositor has no org_kde_kwin_blur_manager")?;
                let object = manager.create(&surface, &qh, ());
                match rects {
                    // No region is the whole surface.
                    None => object.set_region(None),
                    Some(_) => {
                        let region = self.region(rects)?;
                        object.set_region(Some(&region));
                        region.destroy();
                    }
                }
                object.commit();
                Applied::Kde(object)
            }
            BlurPath::X11Property | BlurPath::Dwm => {
                return Err("that is not a Wayland blur path".into())
            }
        };
        self.applied.insert(label.to_string(), applied);
        self.conn.flush().map_err(|error| error.to_string())
    }

    /// Takes away the blur this session put on `label`'s window. `surface` is needed to unset KDE's, which the protocol removes by surface.
    ///
    /// # Safety
    /// `surface`, when given, must be the live `wl_surface` of the window `label` names.
    pub unsafe fn unblur(&mut self, label: &str, surface: Option<RawProxy>) -> Result<(), String> {
        // SAFETY: as for `blur`.
        let surface = surface.and_then(|raw| unsafe { ffi::foreign::<WlSurface>(&self.conn, raw) });
        self.remove(label, surface.as_ref());
        self.conn.flush().map_err(|error| error.to_string())
    }

    fn remove(&mut self, label: &str, surface: Option<&WlSurface>) {
        match self.applied.remove(label) {
            Some(Applied::Ext(object)) => object.destroy(),
            Some(Applied::Kde(object)) => {
                if let (Some(manager), Some(surface)) = (&self.kde, surface) {
                    manager.unset(surface);
                }
                object.release();
            }
            None => {}
        }
    }
}
