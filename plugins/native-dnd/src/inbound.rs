// Turns the runtime's raw drag-drop events into the plugin's normalised ones: logical positions, modifiers, lossless URIs and the self-drop flag
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{path::PathBuf, time::Instant};

use crate::{
    models::{DropEvent, EnterEvent, LeaveEvent, Modifiers, OverEvent, Position},
    outbound::Outbound,
    uri,
};

/// The unit the runtime reports a drag position in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionUnit {
    /// Webview CSS pixels already: Linux, where wry passes GTK's coordinates through.
    Logical,
    /// Physical client-area pixels: Windows.
    Physical,
}

/// Turns a position as the runtime reports it into webview-logical CSS pixels. The one place the conversion happens. A scale that is not positive is treated as 1.
pub fn normalise_position(raw: (f64, f64), unit: PositionUnit, scale_factor: f64) -> Position {
    let scale = if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    };
    match unit {
        PositionUnit::Logical => Position { x: raw.0, y: raw.1 },
        PositionUnit::Physical => Position {
            x: raw.0 / scale,
            y: raw.1 / scale,
        },
    }
}

/// GDK's `GdkModifierType` bits for Shift, Control and Mod1 (Alt).
const GDK_SHIFT_MASK: u32 = 1 << 0;
const GDK_CONTROL_MASK: u32 = 1 << 2;
const GDK_MOD1_MASK: u32 = 1 << 3;

/// Reads the modifiers from a GDK modifier mask.
pub fn modifiers_from_gdk_mask(mask: u32) -> Modifiers {
    Modifiers {
        ctrl: mask & GDK_CONTROL_MASK != 0,
        shift: mask & GDK_SHIFT_MASK != 0,
        alt: mask & GDK_MOD1_MASK != 0,
    }
}

/// A drag-drop event as the runtime delivers it, with the platform's types stripped.
#[derive(Debug, Clone, PartialEq)]
pub enum RawEvent {
    Enter {
        paths: Vec<PathBuf>,
        position: (f64, f64),
    },
    Over {
        position: (f64, f64),
    },
    Drop {
        paths: Vec<PathBuf>,
        position: (f64, f64),
    },
    Leave,
}

/// What the platform knows at the time of an event.
#[derive(Debug, Clone)]
pub struct Env {
    pub unit: PositionUnit,
    pub scale_factor: f64,
    pub modifiers: Modifiers,
    /// The raw `text/uri-list` of the drag, normalised, where the platform has one (Linux). The runtime's own `paths` are lossy for names that are not valid UTF-8 there.
    pub raw_uris: Option<Vec<String>>,
    /// The platform's own sign that the drag began in this process (GTK: a drag source widget exists).
    pub source_is_ours: bool,
    pub now: Instant,
}

/// A normalised event, ready to be sent to a window.
#[derive(Debug, Clone, PartialEq)]
pub enum Translated {
    Enter(EnterEvent),
    Over(OverEvent),
    Drop(DropEvent),
    Leave(LeaveEvent),
}

fn display(path: &std::path::Path) -> String {
    path.to_string_lossy().into_owned()
}

/// The URIs and the display paths of an event's files: from the raw list where there is one, else from the runtime's paths.
fn files(paths: &[PathBuf], raw_uris: &Option<Vec<String>>) -> (Vec<String>, Vec<String>) {
    match raw_uris {
        Some(uris) if !uris.is_empty() => {
            let shown = uris
                .iter()
                .map(|u| uri::uri_to_path(u).map_or_else(|| u.clone(), |p| display(&p)))
                .collect();
            (uris.clone(), shown)
        }
        _ => (
            paths.iter().filter_map(|p| uri::path_to_uri(p)).collect(),
            paths.iter().map(|p| display(p)).collect(),
        ),
    }
}

/// Normalises one event for `window`. A drop is flagged `self_drop` when the platform says the drag began here or its files are those of this process's live (or just finished) outbound drag.
pub fn translate(window: &str, raw: RawEvent, env: &Env, outbound: &Outbound) -> Translated {
    let window = window.to_string();
    let position = |p| normalise_position(p, env.unit, env.scale_factor);
    match raw {
        RawEvent::Enter {
            paths,
            position: at,
        } => {
            let (uris, paths) = files(&paths, &env.raw_uris);
            Translated::Enter(EnterEvent {
                window,
                paths,
                uris,
                position: position(at),
                modifiers: env.modifiers,
            })
        }
        RawEvent::Over { position: at } => Translated::Over(OverEvent {
            window,
            position: position(at),
            modifiers: env.modifiers,
        }),
        RawEvent::Drop {
            paths,
            position: at,
        } => {
            let (uris, paths) = files(&paths, &env.raw_uris);
            let self_drop = env.source_is_ours || outbound.is_self_drop(&uris, env.now);
            Translated::Drop(DropEvent {
                window,
                paths,
                uris,
                position: position(at),
                modifiers: env.modifiers,
                self_drop,
            })
        }
        RawEvent::Leave => Translated::Leave(LeaveEvent { window }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::DragOutcome;

    fn env() -> Env {
        Env {
            unit: PositionUnit::Logical,
            scale_factor: 1.0,
            modifiers: Modifiers::default(),
            raw_uris: None,
            source_is_ours: false,
            now: Instant::now(),
        }
    }

    #[test]
    fn linux_positions_are_already_logical() {
        let at = normalise_position((12.0, 157.0), PositionUnit::Logical, 2.0);
        assert_eq!(at, Position { x: 12.0, y: 157.0 });
    }

    #[test]
    fn windows_positions_are_divided_by_the_scale_factor() {
        let at = normalise_position((192.0, 114.0), PositionUnit::Physical, 1.75);
        assert!((at.x - 109.714_285).abs() < 1e-3, "{at:?}");
        assert!((at.y - 65.142_857).abs() < 1e-3, "{at:?}");
        assert_eq!(
            normalise_position((200.0, 100.0), PositionUnit::Physical, 2.0),
            Position { x: 100.0, y: 50.0 }
        );
    }

    #[test]
    fn a_bad_scale_factor_is_one() {
        for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                normalise_position((10.0, 20.0), PositionUnit::Physical, scale),
                Position { x: 10.0, y: 20.0 }
            );
        }
    }

    #[test]
    fn gdk_masks_map_to_modifiers() {
        assert_eq!(modifiers_from_gdk_mask(0), Modifiers::default());
        assert_eq!(
            modifiers_from_gdk_mask(GDK_CONTROL_MASK),
            Modifiers {
                ctrl: true,
                shift: false,
                alt: false
            }
        );
        assert_eq!(
            modifiers_from_gdk_mask(GDK_SHIFT_MASK | GDK_MOD1_MASK),
            Modifiers {
                ctrl: false,
                shift: true,
                alt: true
            }
        );
        // Caps Lock (bit 1), Mod2 (Num Lock, bit 4) and the button masks are not modifiers.
        assert_eq!(
            modifiers_from_gdk_mask(1 << 1 | 1 << 4 | 1 << 8 | 1 << 9),
            Modifiers::default()
        );
        assert_eq!(
            modifiers_from_gdk_mask(0xffff_ffff),
            Modifiers {
                ctrl: true,
                shift: true,
                alt: true
            }
        );
    }

    #[test]
    fn an_enter_prefers_the_raw_uris_and_shows_decoded_paths() {
        let mut env = env();
        env.modifiers = Modifiers {
            ctrl: true,
            shift: false,
            alt: false,
        };
        env.raw_uris = Some(vec![
            "file:///t/bad%FF%FE.txt".into(),
            "file:///t/with%20space".into(),
        ]);
        let lossy = vec![
            PathBuf::from("/t/bad\u{fffd}\u{fffd}.txt"),
            PathBuf::from("/t/with space"),
        ];
        let out = translate(
            "main-1",
            RawEvent::Enter {
                paths: lossy,
                position: (5.0, 6.0),
            },
            &env,
            &Outbound::default(),
        );
        let Translated::Enter(enter) = out else {
            panic!("{out:?}")
        };
        assert_eq!(enter.window, "main-1");
        assert_eq!(
            enter.uris,
            vec![
                "file:///t/bad%FF%FE.txt".to_string(),
                "file:///t/with%20space".to_string()
            ]
        );
        // Windows spells the same path with backslashes.
        let sep = if cfg!(windows) { '\\' } else { '/' };
        assert_eq!(enter.paths[1], format!("{sep}t{sep}with space"));
        assert_eq!(
            enter.paths[0],
            format!("{sep}t{sep}bad\u{fffd}\u{fffd}.txt")
        );
        assert_eq!(enter.position, Position { x: 5.0, y: 6.0 });
        assert!(enter.modifiers.ctrl);
    }

    #[cfg(unix)]
    #[test]
    fn without_raw_uris_the_uris_come_from_the_paths() {
        let out = translate(
            "w",
            RawEvent::Enter {
                paths: vec![PathBuf::from("/t/a b#.txt")],
                position: (0.0, 0.0),
            },
            &env(),
            &Outbound::default(),
        );
        let Translated::Enter(enter) = out else {
            panic!("{out:?}")
        };
        assert_eq!(enter.uris, vec!["file:///t/a%20b%23.txt".to_string()]);
        assert_eq!(enter.paths, vec!["/t/a b#.txt".to_string()]);
    }

    #[test]
    fn over_and_leave_carry_the_window() {
        let mut env = env();
        env.unit = PositionUnit::Physical;
        env.scale_factor = 2.0;
        let over = translate(
            "w",
            RawEvent::Over {
                position: (40.0, 20.0),
            },
            &env,
            &Outbound::default(),
        );
        assert_eq!(
            over,
            Translated::Over(OverEvent {
                window: "w".into(),
                position: Position { x: 20.0, y: 10.0 },
                modifiers: Modifiers::default()
            })
        );
        assert_eq!(
            translate("w", RawEvent::Leave, &env, &Outbound::default()),
            Translated::Leave(LeaveEvent { window: "w".into() })
        );
    }

    #[test]
    fn a_drop_of_an_outside_file_is_not_a_self_drop() {
        let mut env = env();
        env.raw_uris = Some(vec!["file:///t/a".into()]);
        let out = translate(
            "w",
            RawEvent::Drop {
                paths: vec![PathBuf::from("/t/a")],
                position: (1.0, 2.0),
            },
            &env,
            &Outbound::default(),
        );
        let Translated::Drop(drop) = out else {
            panic!("{out:?}")
        };
        assert!(!drop.self_drop);
        assert_eq!(drop.uris, vec!["file:///t/a".to_string()]);
    }

    #[test]
    fn a_drop_of_the_live_outbound_drag_is_a_self_drop_with_the_same_uris() {
        let outbound = Outbound::default();
        let offered = vec!["file:///t/a".to_string(), "file:///t/b%20c".to_string()];
        let id = outbound.begin(&offered).unwrap();
        let mut env = env();
        env.raw_uris = Some(vec!["file:///t/b%20c".into(), "file:///t/a".into()]);
        let drop = |env: &Env| {
            let Translated::Drop(drop) = translate(
                "w",
                RawEvent::Drop {
                    paths: vec![],
                    position: (0.0, 0.0),
                },
                env,
                &outbound,
            ) else {
                panic!("not a drop")
            };
            drop
        };
        assert!(drop(&env).self_drop);

        // The drag has just ended: still its own drop.
        outbound.finish(id, DragOutcome::DroppedCopy, None, env.now);
        assert!(drop(&env).self_drop);

        // Other files while it ran are not.
        env.raw_uris = Some(vec!["file:///t/other".into()]);
        assert!(!drop(&env).self_drop);
    }

    #[test]
    fn the_platforms_own_source_sign_marks_a_self_drop() {
        let mut env = env();
        env.source_is_ours = true;
        env.raw_uris = Some(vec!["file:///t/a".into()]);
        let out = translate(
            "w",
            RawEvent::Drop {
                paths: vec![],
                position: (0.0, 0.0),
            },
            &env,
            &Outbound::default(),
        );
        let Translated::Drop(drop) = out else {
            panic!("{out:?}")
        };
        assert!(drop.self_drop);
    }
}
