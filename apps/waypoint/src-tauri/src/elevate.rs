// Adapts the elevate plugin to the elevated provider's launcher, and fixes where the helper lives
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `waypoint-elevated` speaks to a helper over a stream and knows nothing of how it is started;
// `tauri-plugin-elevate` starts a helper through the system's prompt and knows nothing of what it
// is for (A154). This is where they meet: `ElevateLauncher` is what `ElevatedProvider::connect`
// calls, and it blocks for as long as the person takes at the prompt, giving the wait up when the
// connecting call is cancelled. Nothing else in the app can start the helper, and the page cannot
// ask for it: the plugin has no command for it.

use std::io::{self, Write};

use tauri_plugin_elevate::{Config, ElevateExt, ElevatedStream, LaunchError};
use waypoint_elevated::{Launcher, Transport, READY_LINE};
use waypoint_path::ELEVATED_SCHEME;
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::CancelToken;

use crate::connections::AppCell;

/// Where the packages install the helper: root-owned, in a root-owned folder. The polkit policy
/// names this same path, so no other program can be started through its action.
pub const HELPER_PATH: &str = "/usr/libexec/waypoint/waypoint-elevate-helper";

/// Where the packages install the polkit policy that authorises the helper.
pub const POLICY_PATH: &str = "/usr/share/polkit-1/actions/ca.liminalhq.waypoint.admin.policy";

/// The plugin's configuration: which helper it may start, the policy that authorises it and the
/// line the helper writes once it is running.
pub fn config() -> Config {
    Config::new(HELPER_PATH, POLICY_PATH, READY_LINE)
}

/// Whether the system can start the helper now, read again at each call. The Experimental switch
/// registers the `admin` scheme only while this is true.
pub fn available<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> bool {
    app.elevate().status().available
}

/// The `admin:` location as an error that belongs to the whole connection and not to a path.
fn root() -> Location {
    Location::new("/", format!("{ELEVATED_SCHEME}:///"))
}

/// What a start that did not give a helper means to the person, without a path in any message.
/// A dismissed dialog and an abandoned wait are a cancel; a refusal reads as a failed sign-in
/// (polkit reports a cancelled dialog on some desktops the same way as a refusal, so the page
/// says "cancelled or refused"); and a system that cannot offer elevation says why.
pub fn map_launch_error(error: LaunchError) -> VfsError {
    match error {
        LaunchError::Cancelled | LaunchError::Dismissed => VfsError::Cancelled,
        LaunchError::NotAuthorised => VfsError::AuthFailed { location: root() },
        LaunchError::Unavailable { reason } => VfsError::Unsupported { what: reason },
        LaunchError::Failed { .. } | LaunchError::Io { .. } => VfsError::Io {
            message: error.to_string(),
            location: None,
        },
    }
}

/// The helper's input, holding the process it belongs to. Dropping it closes the input first,
/// which ends the helper, and only then releases the process (the stream's own order).
struct Held {
    writer: Box<dyn Write + Send>,
    _stream: ElevatedStream,
}

impl Write for Held {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.writer.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

/// The stream of a helper that is running, as the provider's transport. The process stays with the
/// writing half, which the connection drops when it closes, so the helper outlives neither.
fn transport(mut stream: ElevatedStream) -> Transport {
    let reader = std::mem::replace(&mut stream.reader, Box::new(io::empty()));
    let writer = std::mem::replace(&mut stream.writer, Box::new(io::sink()));
    Transport {
        reader,
        writer: Box::new(Held {
            writer,
            _stream: stream,
        }),
    }
}

/// Starts the helper through the elevate plugin, reached through the app once it exists (the
/// provider is built before the app).
pub struct ElevateLauncher {
    app: AppCell,
}

impl ElevateLauncher {
    pub fn new(app: AppCell) -> Self {
        Self { app }
    }
}

impl Launcher for ElevateLauncher {
    fn launch(&self, cancel: &CancelToken) -> Result<Transport, VfsError> {
        let Some(app) = self.app.get() else {
            return Err(VfsError::Unsupported {
                what: "administrator access is not ready yet".to_owned(),
            });
        };
        // Blocks until the helper is running, however long the prompt takes.
        let stream = app
            .elevate()
            .launch(&|| cancel.is_cancelled())
            .map_err(map_launch_error)?;
        Ok(transport(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dismissed_dialog_and_an_abandoned_wait_are_a_cancel() {
        assert_eq!(
            map_launch_error(LaunchError::Dismissed),
            VfsError::Cancelled
        );
        assert_eq!(
            map_launch_error(LaunchError::Cancelled),
            VfsError::Cancelled
        );
    }

    #[test]
    fn a_refusal_is_a_failed_sign_in_for_the_whole_connection() {
        assert_eq!(
            map_launch_error(LaunchError::NotAuthorised),
            VfsError::AuthFailed {
                location: Location::new("/", "admin:///")
            }
        );
    }

    #[test]
    fn a_system_that_cannot_elevate_says_why() {
        let error = map_launch_error(LaunchError::Unavailable {
            reason: "pkexec is not installed".to_owned(),
        });
        assert_eq!(
            error,
            VfsError::Unsupported {
                what: "pkexec is not installed".to_owned()
            }
        );
    }

    #[test]
    fn a_failed_start_is_an_io_error_with_no_path() {
        for error in [
            LaunchError::Failed { code: Some(3) },
            LaunchError::Failed { code: None },
            LaunchError::Io {
                kind: io::ErrorKind::NotFound,
            },
        ] {
            let VfsError::Io { message, location } = map_launch_error(error) else {
                panic!("not an I/O error");
            };
            assert_eq!(location, None);
            assert!(!message.contains('/'), "{message}");
            assert!(!message.is_empty());
        }
    }

    #[test]
    fn the_helper_is_named_once_in_the_configuration_the_plugin_gets() {
        let config = config();
        assert_eq!(config.helper, std::path::Path::new(HELPER_PATH));
        assert_eq!(config.policy, std::path::Path::new(POLICY_PATH));
        assert_eq!(config.ready_line, READY_LINE);
        assert!(config.helper.is_absolute() && config.policy.is_absolute());
    }

    #[test]
    fn a_launch_before_the_app_exists_starts_nothing() {
        let launcher = ElevateLauncher::new(AppCell::default());
        assert!(matches!(
            launcher.launch(&CancelToken::new()),
            Err(VfsError::Unsupported { .. })
        ));
    }
}
