// Exercises the plugin through Tauri's mock runtime over a fake backend: the status, the gating of each request by the environment, region and inset validation and the calls that reach the backend
//
// The mock runtime has no capability file, so the IPC layer would refuse plugin commands; these tests call the same `WindowEffects` methods the commands delegate to, and check the JSON the commands would send. Nothing here touches a real window system.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::{Arc, Mutex};

use serde_json::json;
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::{App, AppHandle, Runtime, WebviewUrl, WebviewWindowBuilder, Window};
use tauri_plugin_window_effects::{
    Backend, BoxFuture, Desktop, EffectKind, Effects, Environment, Insets, Reason, Rect,
    SessionType, WindowEffectsError, WindowEffectsExt, FEATURE_MICA, FEATURE_SHADOW_INSET,
};

type Calls = Arc<Mutex<Vec<String>>>;

/// A backend over an environment the test picks, that records the calls it gets.
struct Fake {
    env: Environment,
    calls: Calls,
}

impl<R: Runtime> Backend<R> for Fake {
    fn environment(&self, _app: &AppHandle<R>) -> BoxFuture<'_, Environment> {
        let env = self.env.clone();
        Box::pin(async move { env })
    }

    fn logical_size(&self, _window: &Window<R>) -> Result<(i64, i64), WindowEffectsError> {
        Ok((800, 600))
    }

    fn apply(
        &self,
        window: Window<R>,
        _env: Environment,
        effects: Effects,
    ) -> BoxFuture<'_, Result<(), WindowEffectsError>> {
        let call = format!(
            "apply {} {:?} dark={} region={:?}",
            window.label(),
            effects.kind,
            effects.dark,
            effects.region.map(|region| region.len())
        );
        self.calls.lock().unwrap().push(call);
        Box::pin(async { Ok(()) })
    }

    fn clear(
        &self,
        window: Window<R>,
        _env: Environment,
    ) -> BoxFuture<'_, Result<(), WindowEffectsError>> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("clear {}", window.label()));
        Box::pin(async { Ok(()) })
    }

    fn set_shadow_inset(
        &self,
        window: Window<R>,
        insets: Insets,
    ) -> BoxFuture<'_, Result<(), WindowEffectsError>> {
        self.calls.lock().unwrap().push(format!(
            "inset {} {} {} {} {}",
            window.label(),
            insets.top,
            insets.right,
            insets.bottom,
            insets.left
        ));
        Box::pin(async { Ok(()) })
    }
}

fn gnome_wayland() -> Environment {
    Environment {
        session_type: SessionType::Wayland,
        desktop: Desktop::Gnome,
        has_composite: true,
        wayland_globals: vec!["wl_compositor".into()],
        build_number: None,
    }
}

fn kde_wayland() -> Environment {
    Environment {
        wayland_globals: vec!["org_kde_kwin_blur_manager".into()],
        desktop: Desktop::Kde,
        ..gnome_wayland()
    }
}

fn windows_11() -> Environment {
    Environment {
        session_type: SessionType::Windows,
        desktop: Desktop::Other,
        has_composite: true,
        wayland_globals: Vec::new(),
        build_number: Some(22631),
    }
}

struct Fixture {
    app: App<MockRuntime>,
    calls: Calls,
}

fn fixture(env: Environment) -> Fixture {
    let calls = Calls::default();
    let app = mock_builder()
        .plugin(tauri_plugin_window_effects::init_with(Arc::new(Fake {
            env,
            calls: calls.clone(),
        })))
        .build(mock_context(noop_assets()))
        .expect("the plugin should initialise");
    WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
        .build()
        .expect("a mock window");
    Fixture { app, calls }
}

fn blur(region: Option<Vec<Rect>>) -> Effects {
    Effects {
        kind: EffectKind::Blur,
        dark: true,
        region,
    }
}

fn effects(kind: EffectKind) -> Effects {
    Effects {
        kind,
        dark: false,
        region: None,
    }
}

#[tokio::test]
async fn the_status_is_the_environments_and_serialises_with_typed_reasons() {
    let fx = fixture(gnome_wayland());
    let status = fx.app.window_effects().get_status().await;
    assert!(status.has("opacity"));
    let json = serde_json::to_value(&status).unwrap();
    let blur = json["features"]
        .as_array()
        .unwrap()
        .iter()
        .find(|feature| feature["name"] == "blur")
        .unwrap()
        .clone();
    assert_eq!(blur["available"], json!(false));
    assert_eq!(blur["reason"], json!("compositor-has-no-blur"));
    assert_eq!(json["flavour"], json!("wayland"));
}

#[tokio::test]
async fn a_blur_the_compositor_can_do_reaches_the_backend() {
    let fx = fixture(kde_wayland());
    fx.app
        .window_effects()
        .apply("main", blur(None))
        .await
        .expect("blur applies");
    assert_eq!(
        *fx.calls.lock().unwrap(),
        ["apply main Blur dark=true region=None"]
    );
}

#[tokio::test]
async fn an_effect_the_system_cannot_do_is_a_typed_unsupported_error_and_never_reaches_the_backend()
{
    let gnome = fixture(gnome_wayland());
    let error = gnome
        .app
        .window_effects()
        .apply("main", blur(None))
        .await
        .unwrap_err();
    assert!(matches!(
        &error,
        WindowEffectsError::Unsupported {
            reason: Reason::CompositorHasNoBlur,
            message
        } if message.contains("GNOME")
    ));
    assert_eq!(
        serde_json::to_value(&error).unwrap()["kind"],
        json!("unsupported")
    );
    let mica = gnome
        .app
        .window_effects()
        .apply("main", effects(EffectKind::Mica))
        .await
        .unwrap_err();
    assert!(matches!(
        mica,
        WindowEffectsError::Unsupported {
            reason: Reason::WindowsOnly,
            ..
        }
    ));
    assert!(gnome.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn mica_applies_on_windows_11() {
    let fx = fixture(windows_11());
    assert!(fx.app.window_effects().get_status().await.has(FEATURE_MICA));
    fx.app
        .window_effects()
        .apply("main", effects(EffectKind::Mica))
        .await
        .unwrap();
    assert_eq!(
        *fx.calls.lock().unwrap(),
        ["apply main Mica dark=false region=None"]
    );
}

#[tokio::test]
async fn an_unknown_window_is_a_typed_error() {
    let fx = fixture(kde_wayland());
    let error = fx
        .app
        .window_effects()
        .apply("nobody", blur(None))
        .await
        .unwrap_err();
    assert_eq!(
        error,
        WindowEffectsError::WindowNotFound {
            label: "nobody".into()
        }
    );
    assert!(fx.app.window_effects().clear("nobody").await.is_err());
}

#[tokio::test]
async fn a_region_outside_the_window_is_refused_before_the_backend() {
    let fx = fixture(kde_wayland());
    let inside = Rect {
        x: 0,
        y: 0,
        width: 10,
        height: 10,
    };
    let outside = Rect {
        x: 0,
        y: 0,
        width: 100_000,
        height: 10,
    };
    let empty = Rect { width: 0, ..inside };
    for region in [vec![], vec![outside], vec![inside, empty]] {
        let error = fx
            .app
            .window_effects()
            .apply("main", blur(Some(region)))
            .await
            .unwrap_err();
        assert!(
            matches!(error, WindowEffectsError::InvalidRegion { .. }),
            "{error:?}"
        );
    }
    assert!(fx.calls.lock().unwrap().is_empty());
    fx.app
        .window_effects()
        .apply("main", blur(Some(vec![inside])))
        .await
        .unwrap();
    assert_eq!(fx.calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn none_clears_and_clear_clears() {
    let fx = fixture(kde_wayland());
    fx.app
        .window_effects()
        .apply("main", effects(EffectKind::None))
        .await
        .unwrap();
    fx.app.window_effects().clear("main").await.unwrap();
    assert_eq!(*fx.calls.lock().unwrap(), ["clear main", "clear main"]);
}

#[tokio::test]
async fn the_shadow_inset_is_checked_then_passed_on() {
    let fx = fixture(gnome_wayland());
    assert!(fx
        .app
        .window_effects()
        .get_status()
        .await
        .has(FEATURE_SHADOW_INSET));
    let negative = Insets {
        top: -1,
        right: 0,
        bottom: 0,
        left: 0,
    };
    assert!(matches!(
        fx.app
            .window_effects()
            .set_shadow_inset("main", negative)
            .await,
        Err(WindowEffectsError::InvalidInsets { .. })
    ));
    let insets = Insets {
        top: 8,
        right: 8,
        bottom: 8,
        left: 8,
    };
    fx.app
        .window_effects()
        .set_shadow_inset("main", insets)
        .await
        .unwrap();
    assert_eq!(*fx.calls.lock().unwrap(), ["inset main 8 8 8 8"]);
}

#[tokio::test]
async fn the_shadow_inset_is_unsupported_on_windows() {
    let fx = fixture(windows_11());
    let error = fx
        .app
        .window_effects()
        .set_shadow_inset(
            "main",
            Insets {
                top: 1,
                right: 1,
                bottom: 1,
                left: 1,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        WindowEffectsError::Unsupported {
            reason: Reason::GtkOnly,
            ..
        }
    ));
}

#[tokio::test]
async fn an_unsupported_system_refuses_every_request_with_a_typed_error() {
    let fx = fixture(Environment::unsupported());
    let status = fx.app.window_effects().get_status().await;
    assert!(!status.available);
    assert!(fx.app.window_effects().clear("main").await.is_err());
    let error = fx
        .app
        .window_effects()
        .apply("main", effects(EffectKind::None))
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        WindowEffectsError::Unsupported {
            reason: Reason::UnsupportedPlatform,
            ..
        }
    ));
    assert!(fx.calls.lock().unwrap().is_empty());
}
