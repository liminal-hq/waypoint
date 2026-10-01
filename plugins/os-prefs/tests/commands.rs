// Exercises the plugin through Tauri's mock runtime, without a desktop session
//
// The mock runtime has no capability file, so the IPC layer would refuse plugin commands; these
// tests call the same `OsPrefs` methods the commands delegate to.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg(desktop)]

use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::App;
use tauri_plugin_os_prefs::OsPrefsExt;
#[cfg(target_os = "linux")]
use tauri_plugin_os_prefs::TimeFormatSource;

fn app() -> App<MockRuntime> {
    // An unrecognised desktop reads neither a portal nor a settings tool, so the answer comes
    // from the process locale and does not depend on the machine running the tests.
    std::env::set_var("XDG_CURRENT_DESKTOP", "test-desktop");
    mock_builder()
        .plugin(tauri_plugin_os_prefs::init())
        .build(mock_context(noop_assets()))
        .expect("the plugin should initialise")
}

#[tokio::test(flavor = "multi_thread")]
async fn the_time_format_comes_with_a_source() {
    let app = app();
    let format = app.os_prefs().get_time_format().await.unwrap();
    #[cfg(target_os = "linux")]
    assert!(
        matches!(
            format.source,
            TimeFormatSource::Locale | TimeFormatSource::Default
        ),
        "an unrecognised desktop uses the locale: {format:?}"
    );
    let json = serde_json::to_value(format).unwrap();
    assert!(json["is24Hour"].is_boolean(), "{json}");
    assert!(json["source"].is_string(), "{json}");
}

#[tokio::test(flavor = "multi_thread")]
async fn desktops_report_the_neutral_android_only_answers() {
    let app = app();
    assert_eq!(
        app.os_prefs()
            .get_animator_duration_scale()
            .await
            .unwrap()
            .scale,
        1.0
    );
    app.os_prefs().open_notification_settings().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_status_lists_every_feature_and_says_why_one_is_missing() {
    let app = app();
    let status = app.os_prefs().status().await.unwrap();
    let names: Vec<_> = status.features.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "timeFormat",
            "timeFormatWatch",
            "animatorDurationScale",
            "notificationSettings"
        ]
    );
    for feature in &status.features {
        if !feature.available {
            assert!(feature.reason.is_some(), "{feature:?}");
        }
    }
    assert!(status.available);
    assert!(status.time_format_source.is_some());
}
