// Desktop implementation: reads the time format from the platform and watches for changes
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Manager, Runtime};

use crate::{
    models::{
        AnimatorDurationScaleResponse, FeatureStatus, PluginStatus, TimeFormat, TimeFormatSource,
    },
    service::{Service, WatchState},
};

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<OsPrefs<R>> {
    app.manage(Service::default());
    app.state::<Service>().start(app);
    Ok(OsPrefs(app.clone()))
}

/// Access to the OsPrefs APIs
pub struct OsPrefs<R: Runtime>(AppHandle<R>);

impl<R: Runtime> OsPrefs<R> {
    /// Reads the user's 12/24-hour preference now. A reading that differs from the previous one
    /// also emits the change event.
    pub async fn get_time_format(&self) -> crate::Result<TimeFormat> {
        Ok(self.0.state::<Service>().refresh(&self.0).await)
    }

    /// Desktops have no animator duration scale: always full speed.
    pub async fn get_animator_duration_scale(
        &self,
    ) -> crate::Result<AnimatorDurationScaleResponse> {
        Ok(AnimatorDurationScaleResponse { scale: 1.0 })
    }

    /// Not an Android concept; nothing to open.
    pub async fn open_notification_settings(&self) -> crate::Result<()> {
        Ok(())
    }

    /// Reports which features work on this system and why the others do not.
    pub async fn status(&self) -> crate::Result<PluginStatus> {
        let service = self.0.state::<Service>();
        let format = service.refresh(&self.0).await;
        let time_format = match (format.source, service.note()) {
            (TimeFormatSource::Default, note) => FeatureStatus::unavailable(
                "timeFormat",
                note.unwrap_or_else(|| "no source could be read".to_string()),
            ),
            (_, Some(note)) => FeatureStatus::degraded("timeFormat", note),
            (_, None) => FeatureStatus::available("timeFormat"),
        };
        let watch = match service.watch_state() {
            WatchState::Listening => FeatureStatus::available("timeFormatWatch"),
            WatchState::Starting => FeatureStatus::unavailable(
                "timeFormatWatch",
                "the change watcher is still starting",
            ),
            WatchState::Unavailable(reason) => {
                FeatureStatus::unavailable("timeFormatWatch", reason)
            }
        };
        Ok(PluginStatus::new(
            vec![
                time_format,
                watch,
                FeatureStatus::unavailable(
                    "animatorDurationScale",
                    "Android only; desktops always report a scale of 1",
                ),
                FeatureStatus::unavailable("notificationSettings", "Android only"),
            ],
            Some(format.source),
        ))
    }
}
