// Maps the platform and the runtime probe results to the status the plugin reports
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::models::{
    PluginStatus, UnavailableFeature, FEATURE_CURSOR_FOLLOW, FEATURE_GHOST, FEATURE_HIT_TEST,
    FEATURE_WINDOW_POSITION,
};

/// The windowing system the app is running under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    X11,
    Wayland,
    Windows,
    Unsupported,
}

impl Platform {
    /// Whether the platform's own claims are trusted without probing: Windows reports real positions and a live cursor.
    pub fn trusts_probes(self) -> bool {
        matches!(self, Platform::Windows)
    }
}

/// What the plugin learned about this system at runtime, cached for the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Probes {
    pub platform: Platform,
    pub ghost_created: bool,
    /// A position that was set read back as the same move.
    pub window_position: bool,
    /// Another window's inner position can be read.
    pub hit_test: bool,
}

impl Probes {
    /// The result for a platform that is trusted without probing.
    pub fn trusted(platform: Platform, ghost_created: bool) -> Self {
        Self {
            platform,
            ghost_created,
            window_position: true,
            hit_test: true,
        }
    }
}

/// True if two requested positions read back as the same move.
///
/// Only the difference between the two reads is compared, so a window manager's own offset (a frame, a shadow inset) cancels out, while a system that ignores positions and reports `(0, 0)` for everything produces no difference and fails.
pub fn position_probe_passes(requested: [(i32, i32); 2], read_back: [(i32, i32); 2]) -> bool {
    let wanted = (
        requested[1].0 - requested[0].0,
        requested[1].1 - requested[0].1,
    );
    let got = (
        read_back[1].0 - read_back[0].0,
        read_back[1].1 - read_back[0].1,
    );
    wanted != (0, 0) && wanted == got
}

/// Whether windows can be hit-tested: positions are readable if the ghost's moved when asked, or if another window reports a position other than the origin.
pub fn hit_test_probe(window_position_works: bool, other_inner_positions: &[(i32, i32)]) -> bool {
    window_position_works
        || other_inner_positions
            .iter()
            .any(|&position| position != (0, 0))
}

fn cursor_follow_reason(platform: Platform) -> Option<&'static str> {
    match platform {
        Platform::X11 | Platform::Windows => None,
        Platform::Wayland => Some(
            "the compositor reports the cursor as (0, 0) and does not expose its position to apps",
        ),
        Platform::Unsupported => Some("window tear-off is not supported on this platform"),
    }
}

/// The status for what is known about this system.
///
/// `cursor_follow` is a platform claim: the follow loop still checks that the value changes, because the cursor is only live on X11 while a button is held.
pub fn status_for(probes: &Probes) -> PluginStatus {
    let follow_reason = cursor_follow_reason(probes.platform);
    let position_reason = (!probes.window_position)
        .then_some("the windowing system ignores window positions or reports them as (0, 0)");
    let hit_reason = (!probes.hit_test).then_some("no window reports a readable position");
    let ghost_reason = if !probes.ghost_created {
        Some("the ghost window could not be created")
    } else if !probes.window_position {
        Some("the ghost cannot be placed under the cursor")
    } else {
        None
    };

    let entries = [
        (FEATURE_GHOST, ghost_reason),
        (FEATURE_CURSOR_FOLLOW, follow_reason),
        (FEATURE_WINDOW_POSITION, position_reason),
        (FEATURE_HIT_TEST, hit_reason),
    ];
    let features = entries
        .iter()
        .filter(|(_, reason)| reason.is_none())
        .map(|(name, _)| (*name).to_string())
        .collect::<Vec<_>>();
    let unavailable = entries
        .iter()
        .filter_map(|(name, reason)| {
            reason.map(|reason| UnavailableFeature {
                feature: (*name).to_string(),
                reason: reason.to_string(),
            })
        })
        .collect::<Vec<_>>();
    let available = !features.is_empty();
    PluginStatus {
        available,
        reason: (!available).then(|| {
            unavailable
                .iter()
                .find(|entry| entry.feature == FEATURE_CURSOR_FOLLOW)
                .or(unavailable.first())
                .map(|entry| entry.reason.clone())
                .unwrap_or_default()
        }),
        features,
        unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feature_names(status: &PluginStatus) -> Vec<&str> {
        status.features.iter().map(String::as_str).collect()
    }

    #[test]
    fn x11_with_working_probes_reports_everything() {
        let status = status_for(&Probes {
            platform: Platform::X11,
            ghost_created: true,
            window_position: true,
            hit_test: true,
        });
        assert!(status.available);
        assert_eq!(status.reason, None);
        assert_eq!(
            feature_names(&status),
            ["ghost", "cursor_follow", "window_position", "hit_test"]
        );
        assert!(status.unavailable.is_empty());
    }

    #[test]
    fn wayland_reports_nothing_with_reasons() {
        let status = status_for(&Probes {
            platform: Platform::Wayland,
            ghost_created: true,
            window_position: false,
            hit_test: false,
        });
        assert!(!status.available);
        assert!(status.features.is_empty());
        assert_eq!(status.unavailable.len(), 4);
        assert!(status.reason.as_deref().unwrap().contains("(0, 0)"));
        assert!(status
            .unavailable
            .iter()
            .all(|entry| !entry.reason.is_empty()));
    }

    #[test]
    fn windows_is_trusted_and_reports_everything() {
        let probes = Probes::trusted(Platform::Windows, true);
        assert!(Platform::Windows.trusts_probes());
        assert!(!Platform::X11.trusts_probes());
        assert_eq!(status_for(&probes).features.len(), 4);
    }

    #[test]
    fn unsupported_platforms_report_nothing() {
        let status = status_for(&Probes {
            platform: Platform::Unsupported,
            ghost_created: false,
            window_position: false,
            hit_test: false,
        });
        assert!(!status.available);
        assert_eq!(status.unavailable.len(), 4);
        assert!(status.reason.unwrap().contains("not supported"));
    }

    #[test]
    fn a_failed_ghost_leaves_the_rest_of_x11_working() {
        let status = status_for(&Probes {
            platform: Platform::X11,
            ghost_created: false,
            window_position: true,
            hit_test: true,
        });
        assert!(status.available);
        assert_eq!(
            feature_names(&status),
            ["cursor_follow", "window_position", "hit_test"]
        );
        assert_eq!(status.unavailable[0].feature, "ghost");
    }

    #[test]
    fn the_position_probe_compares_moves_not_places() {
        // A window manager that offsets every position by a frame still passes.
        assert!(position_probe_passes(
            [(64, 64), (104, 84)],
            [(100, 138), (140, 158)]
        ));
        // A system that reports the origin for everything fails.
        assert!(!position_probe_passes(
            [(64, 64), (104, 84)],
            [(0, 0), (0, 0)]
        ));
        // So does one that ignores the second request.
        assert!(!position_probe_passes(
            [(64, 64), (104, 84)],
            [(64, 64), (64, 64)]
        ));
        // Two identical requests prove nothing.
        assert!(!position_probe_passes([(5, 5), (5, 5)], [(0, 0), (0, 0)]));
    }

    #[test]
    fn the_hit_test_probe_accepts_either_signal() {
        assert!(hit_test_probe(true, &[]));
        assert!(hit_test_probe(false, &[(0, 0), (-1920, 40)]));
        assert!(!hit_test_probe(false, &[(0, 0), (0, 0)]));
        assert!(!hit_test_probe(false, &[]));
    }
}
