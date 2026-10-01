// Mobile bridge for reading native OS preferences
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::de::DeserializeOwned;
#[cfg(target_os = "android")]
use serde::Deserialize;
use tauri::{plugin::PluginApi, AppHandle, Runtime};

use crate::models::{
    AnimatorDurationScaleResponse, FeatureStatus, PluginStatus, TimeFormat, TimeFormatSource,
};

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "ca.liminalhq.osprefs";

/// The Android plugin's reply, which predates the `source` field.
#[cfg(target_os = "android")]
#[derive(Deserialize)]
struct AndroidTimeFormat {
    #[serde(rename = "is24Hour")]
    is_24_hour: bool,
}

// Initialize the plugin API
pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> crate::Result<OsPrefs<R>> {
    #[cfg(target_os = "android")]
    let handle = api.register_android_plugin(PLUGIN_IDENTIFIER, "OsPrefsPlugin")?;

    // There is no iOS plugin: iOS reads what it needs from Rust through Foundation.
    #[cfg(not(target_os = "android"))]
    let _ = api;

    Ok(OsPrefs {
        #[cfg(target_os = "android")]
        handle,
        #[cfg(not(target_os = "android"))]
        _runtime: std::marker::PhantomData,
    })
}

/// Access to the OsPrefs APIs
pub struct OsPrefs<R: Runtime> {
    #[cfg(target_os = "android")]
    handle: tauri::plugin::PluginHandle<R>,
    #[cfg(not(target_os = "android"))]
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<R: Runtime> OsPrefs<R> {
    pub async fn get_time_format(&self) -> crate::Result<TimeFormat> {
        #[cfg(target_os = "android")]
        {
            let reply: AndroidTimeFormat = self.handle.run_mobile_plugin("getTimeFormat", ())?;
            Ok(TimeFormat {
                is_24_hour: reply.is_24_hour,
                source: TimeFormatSource::Android,
            })
        }

        #[cfg(target_os = "ios")]
        {
            Ok(match crate::apple::is_24_hour() {
                Ok(is_24_hour) => TimeFormat {
                    is_24_hour,
                    source: TimeFormatSource::Ios,
                },
                Err(reason) => {
                    log::warn!("cannot read the iOS hour format: {reason}");
                    TimeFormat {
                        is_24_hour: false,
                        source: TimeFormatSource::Default,
                    }
                }
            })
        }
    }

    pub async fn get_animator_duration_scale(
        &self,
    ) -> crate::Result<AnimatorDurationScaleResponse> {
        #[cfg(target_os = "android")]
        {
            self.handle
                .run_mobile_plugin("getAnimatorDurationScale", ())
                .map_err(Into::into)
        }

        #[cfg(not(target_os = "android"))]
        {
            Ok(AnimatorDurationScaleResponse { scale: 1.0 })
        }
    }

    /// Opens the OS notification settings screen for this app.
    pub async fn open_notification_settings(&self) -> crate::Result<()> {
        #[cfg(target_os = "android")]
        {
            self.handle
                .run_mobile_plugin("openNotificationSettings", ())
                .map_err(Into::into)
        }

        #[cfg(not(target_os = "android"))]
        {
            Ok(())
        }
    }

    pub async fn status(&self) -> crate::Result<PluginStatus> {
        let android = cfg!(target_os = "android");
        let only_android = "Android only";
        let feature = |name: &str| {
            if android {
                FeatureStatus::available(name)
            } else {
                FeatureStatus::unavailable(name, only_android)
            }
        };
        Ok(PluginStatus::new(
            vec![
                FeatureStatus::available("timeFormat"),
                FeatureStatus::unavailable(
                    "timeFormatWatch",
                    "mobile platforms are read on demand, not pushed",
                ),
                feature("animatorDurationScale"),
                feature("notificationSettings"),
            ],
            Some(if android {
                TimeFormatSource::Android
            } else {
                TimeFormatSource::Ios
            }),
        ))
    }
}
