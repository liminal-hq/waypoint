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

/// Where the packages install the helper on Linux: root-owned, in a root-owned folder. The polkit
/// policy names this same path, so no other program can be started through its action.
#[cfg(not(windows))]
pub const HELPER_PATH: &str = "/usr/libexec/waypoint/waypoint-elevate-helper";

/// Where the packages install the polkit policy that authorises the helper. Windows has none: the
/// prompt is UAC's, and the plugin ignores this path there.
#[cfg(not(windows))]
pub const POLICY_PATH: &str = "/usr/share/polkit-1/actions/ca.liminalhq.waypoint.admin.policy";
#[cfg(windows)]
pub const POLICY_PATH: &str = "";

/// The helper's file name on Windows. The installer puts it beside `waypoint.exe`, under Program
/// Files, and the plugin offers elevation only when it is there.
#[cfg(windows)]
pub const HELPER_FILE: &str = "waypoint-elevate-helper.exe";

/// The start of the names of the pipes of a launch (Windows).
pub const PIPE_PREFIX: &str = "waypoint-elevate";

/// The helper's path: fixed on Linux; beside the running program on Windows. Without a known
/// location for the program the path is relative, which the plugin reports as not installed.
fn helper_path() -> std::path::PathBuf {
    #[cfg(windows)]
    {
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|folder| folder.join(HELPER_FILE)))
            .unwrap_or_else(|| std::path::PathBuf::from(HELPER_FILE))
    }
    #[cfg(not(windows))]
    {
        std::path::PathBuf::from(HELPER_PATH)
    }
}

/// The plugin's configuration: which helper it may start, the policy that authorises it (Linux),
/// the line the helper writes once it is running and the start of the pipe names (Windows).
pub fn config() -> Config {
    Config::new(helper_path(), POLICY_PATH, READY_LINE).with_pipe_prefix(PIPE_PREFIX)
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
        assert_eq!(config.helper, helper_path());
        assert_eq!(config.ready_line, READY_LINE);
        assert_eq!(config.pipe_prefix.as_deref(), Some(PIPE_PREFIX));
        #[cfg(not(windows))]
        {
            assert_eq!(config.helper, std::path::Path::new(HELPER_PATH));
            assert_eq!(config.policy, std::path::Path::new(POLICY_PATH));
            assert!(config.helper.is_absolute() && config.policy.is_absolute());
        }
        #[cfg(windows)]
        assert!(config.helper.ends_with(HELPER_FILE));
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
