// Which entries of the Windows system menu apply to a window in its current state
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// The window facts the system menu's entries depend on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowFacts {
    pub maximised: bool,
    pub minimised: bool,
    /// The window has a sizing border (`WS_THICKFRAME`).
    pub can_size: bool,
    pub can_minimise: bool,
    pub can_maximise: bool,
}

/// Whether each entry that depends on the window's state is enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemMenuState {
    pub restore: bool,
    pub move_window: bool,
    pub size: bool,
    pub minimise: bool,
    pub maximise: bool,
}

/// The state Windows gives its own title bar menu. A menu shown through `TrackPopupMenu` is not
/// refreshed by the system as it would be for a click on the title bar, so its entries keep
/// whatever state they were last left in and must be set to match the window before it is shown.
pub fn system_menu_state(window: WindowFacts) -> SystemMenuState {
    let placed = !window.maximised && !window.minimised;
    SystemMenuState {
        restore: window.maximised || window.minimised,
        move_window: placed,
        size: placed && window.can_size,
        minimise: window.can_minimise && !window.minimised,
        maximise: window.can_maximise && !window.maximised,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(maximised: bool, minimised: bool) -> WindowFacts {
        WindowFacts {
            maximised,
            minimised,
            can_size: true,
            can_minimise: true,
            can_maximise: true,
        }
    }

    #[test]
    fn a_window_in_place_can_move_size_minimise_and_maximise_but_not_restore() {
        assert_eq!(
            system_menu_state(facts(false, false)),
            SystemMenuState {
                restore: false,
                move_window: true,
                size: true,
                minimise: true,
                maximise: true,
            }
        );
    }

    #[test]
    fn a_maximised_window_can_restore_and_minimise_but_not_move_size_or_maximise() {
        assert_eq!(
            system_menu_state(facts(true, false)),
            SystemMenuState {
                restore: true,
                move_window: false,
                size: false,
                minimise: true,
                maximise: false,
            }
        );
    }

    #[test]
    fn a_minimised_window_can_restore_and_maximise_but_not_move_size_or_minimise() {
        assert_eq!(
            system_menu_state(facts(false, true)),
            SystemMenuState {
                restore: true,
                move_window: false,
                size: false,
                minimise: false,
                maximise: true,
            }
        );
    }

    #[test]
    fn a_window_without_the_borders_or_boxes_cannot_use_what_they_give() {
        let state = system_menu_state(WindowFacts {
            can_size: false,
            can_minimise: false,
            can_maximise: false,
            ..facts(false, false)
        });
        assert!(state.move_window && !state.size && !state.minimise && !state.maximise);
    }
}
