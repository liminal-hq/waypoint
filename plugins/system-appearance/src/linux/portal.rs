// Reads and watches titlebar preferences through the xdg-desktop-portal Settings interface
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use ashpd::{
    desktop::settings::Settings,
    zvariant::{OwnedValue, Value},
};
use futures_util::StreamExt;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    models::{DesktopEnvironment, LayoutSource, TitlebarPreferences},
    parse,
};

const NAMESPACE: &str = "org.gnome.desktop.wm.preferences";

/// Extracts a string from a portal value, which may arrive wrapped in a variant.
fn as_string(value: &OwnedValue) -> Option<String> {
    match &**value {
        Value::Str(text) => Some(text.to_string()),
        Value::Value(inner) => match &**inner {
            Value::Str(text) => Some(text.to_string()),
            _ => None,
        },
        _ => None,
    }
}

/// Reads the GNOME window-manager preferences namespace; fails when the layout key is absent.
pub async fn read(desktop_environment: DesktopEnvironment) -> Result<TitlebarPreferences, String> {
    let settings = Settings::new().await.map_err(|error| error.to_string())?;
    let mut namespaces = settings
        .read_all(&[NAMESPACE])
        .await
        .map_err(|error| error.to_string())?;
    let namespace = namespaces.remove(NAMESPACE).unwrap_or_default();
    let value = |key: &str| namespace.get(key).and_then(as_string);

    let layout = value("button-layout").ok_or("the portal does not expose button-layout")?;
    let actions = parse::gnome_actions(
        value("action-double-click-titlebar").as_deref(),
        value("action-middle-click-titlebar").as_deref(),
        value("action-right-click-titlebar").as_deref(),
    );
    Ok(TitlebarPreferences {
        button_layout: parse::gnome_button_layout(&layout),
        actions,
        desktop_environment,
        source: LayoutSource::Portal,
    })
}

/// Aborts the signal-listening task when dropped.
struct TaskGuard(tauri::async_runtime::JoinHandle<()>);

impl Drop for TaskGuard {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Notifies on every portal `SettingChanged` signal in the window-manager namespace.
pub fn watch(changed: UnboundedSender<()>) -> Vec<Box<dyn Send>> {
    let task = tauri::async_runtime::spawn(async move {
        let stream = async {
            let settings = Settings::new().await?;
            settings.receive_setting_changed().await
        };
        let mut stream = match stream.await {
            Ok(stream) => Box::pin(stream),
            Err(error) => {
                log::warn!("system-appearance: cannot listen to the portal: {error}");
                return;
            }
        };
        while let Some(setting) = stream.next().await {
            if setting.namespace() == NAMESPACE && changed.send(()).is_err() {
                break;
            }
        }
    });
    vec![Box::new(TaskGuard(task))]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live smoke test against the real portal; run with `cargo test -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_portal_read() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let preferences = runtime
            .block_on(read(DesktopEnvironment::Gnome))
            .expect("the portal should answer");
        println!("{}", serde_json::to_string_pretty(&preferences).unwrap());
    }
}
