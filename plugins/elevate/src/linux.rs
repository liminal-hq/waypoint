// Starts the helper through pkexec on Linux, after the availability check passes
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::launch::{launch_with, CommandSpawner, ElevatedStream, LaunchError};
use crate::models::PluginStatus;
use crate::polkit::{check, Verdict};
use crate::probe::{Probe, SystemProbe};
use crate::Config;

pub struct Platform {
    config: Config,
    probe: Box<dyn Probe + Send + Sync>,
}

impl Platform {
    pub fn new(config: Config) -> Self {
        Self::with_probe(config, Box::new(SystemProbe))
    }

    /// Checks and launches against an injected system, which tests use.
    pub fn with_probe(config: Config, probe: Box<dyn Probe + Send + Sync>) -> Self {
        Platform { config, probe }
    }

    fn verdict(&self) -> Verdict {
        check(&self.config, self.probe.as_ref())
    }

    pub fn status(&self) -> PluginStatus {
        self.verdict().status
    }

    /// Runs the check again (the system may have changed since the status was read) and refuses when it fails, then starts `pkexec` on the helper.
    pub fn launch(&self, cancelled: &dyn Fn() -> bool) -> Result<ElevatedStream, LaunchError> {
        let verdict = self.verdict();
        let Some(pkexec) = verdict.pkexec else {
            return Err(LaunchError::Unavailable {
                reason: verdict
                    .status
                    .reason
                    .unwrap_or_else(|| "Elevation is not available".to_string()),
            });
        };
        // The helper is named by its path in the policy as well; `pkexec` refuses any other.
        let spawner = CommandSpawner {
            program: pkexec,
            args: vec![self.config.helper.to_string_lossy().into_owned()],
        };
        launch_with(&spawner, &self.config.ready_line, cancelled)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::launch::LaunchError;
    use crate::probe::{FileFacts, Probe};

    /// A system with nothing installed.
    struct Empty;

    impl Probe for Empty {
        fn facts(&self, _: &Path) -> Option<FileFacts> {
            None
        }
        fn facts_following(&self, _: &Path) -> Option<FileFacts> {
            None
        }
        fn env(&self, _: &str) -> Option<String> {
            None
        }
        fn which(&self, _: &str) -> Option<std::path::PathBuf> {
            None
        }
    }

    #[test]
    fn launch_is_refused_with_the_reason_when_the_check_fails() {
        let platform = Platform::with_probe(
            Config::new("/usr/libexec/tool/helper", "/etc/tool.policy", "ready"),
            Box::new(Empty),
        );
        assert!(!platform.status().available);
        match platform.launch(&|| false) {
            Err(LaunchError::Unavailable { reason }) => {
                assert_eq!(reason, crate::polkit::REASON_NO_PKEXEC)
            }
            other => panic!("{:?}", other.map(|_| ())),
        }
    }
}
