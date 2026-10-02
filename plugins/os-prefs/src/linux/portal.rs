// Reads and watches GNOME's clock setting through the xdg-desktop-portal Settings interface
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use ashpd::{
    desktop::settings::Settings,
    zvariant::{OwnedValue, Value},
};
use futures_util::StreamExt;
use tokio::sync::{mpsc::UnboundedSender, oneshot};

use crate::service::ReadySignal;

use crate::parse;

const NAMESPACE: &str = "org.gnome.desktop.interface";
const KEY: &str = "clock-format";

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

/// Reads `clock-format`; fails when the portal is unreachable or does not expose the key.
pub async fn read() -> Result<bool, String> {
    let settings = Settings::new().await.map_err(|error| error.to_string())?;
    let mut namespaces = settings
        .read_all(&[NAMESPACE])
        .await
        .map_err(|error| error.to_string())?;
    let value = namespaces
        .remove(NAMESPACE)
        .and_then(|namespace| namespace.get(KEY).and_then(as_string))
        .ok_or_else(|| format!("the portal does not expose {KEY}"))?;
    parse::gnome_clock_format(&value).ok_or_else(|| format!("unexpected {KEY} value {value:?}"))
}

/// Aborts the signal-listening task when dropped.
struct TaskGuard(tauri::async_runtime::JoinHandle<()>);

impl Drop for TaskGuard {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Notifies on every portal `SettingChanged` signal for the clock key.
///
/// The second value resolves once the signal stream is installed, so the caller can read the
/// baseline only after a change can no longer slip past it. It resolves with the reason, rather
/// than hanging, if the portal cannot be reached.
pub fn watch(changed: UnboundedSender<()>) -> (Vec<Box<dyn Send>>, ReadySignal) {
    let (ready_tx, ready_rx) = oneshot::channel();
    let task = tauri::async_runtime::spawn(async move {
        // The stream borrows the proxy, so the proxy lives as long as the task.
        let settings = match Settings::new().await {
            Ok(settings) => settings,
            Err(error) => {
                log::warn!("cannot listen to the portal: {error}");
                let _ = ready_tx.send(Err(format!("portal: {error}")));
                return;
            }
        };
        let mut stream = match settings.receive_setting_changed().await {
            Ok(stream) => Box::pin(stream),
            Err(error) => {
                log::warn!("cannot listen to the portal: {error}");
                let _ = ready_tx.send(Err(format!("portal: {error}")));
                return;
            }
        };
        let _ = ready_tx.send(Ok(()));
        while let Some(setting) = stream.next().await {
            if setting.namespace() == NAMESPACE && setting.key() == KEY && changed.send(()).is_err()
            {
                break;
            }
        }
    });
    (vec![Box::new(TaskGuard(task))], ready_rx)
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
        let is_24_hour = runtime.block_on(read()).expect("the portal should answer");
        println!("is24Hour = {is_24_hour}");
    }
}
