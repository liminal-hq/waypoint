// The order in which a drag's end-of-drag events are reported, free of any Wayland types
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::event::Event;

/// What the data source tells the drag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// `wl_data_source.target`: a type the target accepts, or none.
    Target(Option<String>),
    /// `wl_data_source.dnd_drop_performed`.
    DropPerformed,
    /// `wl_data_source.dnd_finished`.
    Finished,
    /// `wl_data_source.cancelled`.
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Dragging,
    Dropped,
    Ended,
}

/// Turns the source's events into the ones callers see. A compositor may send `cancelled` right
/// after `dnd_finished`, or any end event twice; only the first end counts.
#[derive(Debug)]
pub struct Machine {
    phase: Phase,
    accepted: bool,
}

impl Machine {
    /// A drag the compositor has just taken; its first event is `Started`.
    pub fn start() -> (Self, Event) {
        (
            Self {
                phase: Phase::Dragging,
                accepted: false,
            },
            Event::Started,
        )
    }

    pub fn is_ended(&self) -> bool {
        self.phase == Phase::Ended
    }

    pub fn feed(&mut self, input: Input) -> Vec<Event> {
        match (self.phase, input) {
            (Phase::Ended, _) => Vec::new(),
            (Phase::Dragging, Input::Target(mime)) => {
                let accepted = mime.is_some();
                if accepted == self.accepted {
                    return Vec::new();
                }
                self.accepted = accepted;
                vec![Event::Target { accepted }]
            }
            // A target is only reported while the button is held.
            (Phase::Dropped, Input::Target(_)) => Vec::new(),
            (Phase::Dragging, Input::DropPerformed) => {
                self.phase = Phase::Dropped;
                vec![Event::DropPerformed]
            }
            (Phase::Dropped, Input::DropPerformed) => Vec::new(),
            (Phase::Dragging, Input::Finished) => {
                // `dnd_finished` without a `dnd_drop_performed` first: report the drop all the same.
                self.phase = Phase::Ended;
                vec![Event::DropPerformed, Event::Finished]
            }
            (Phase::Dropped, Input::Finished) => {
                self.phase = Phase::Ended;
                vec![Event::Finished]
            }
            (phase, Input::Cancelled) => {
                self.phase = Phase::Ended;
                vec![Event::Cancelled {
                    after_drop: phase == Phase::Dropped,
                }]
            }
        }
    }

    /// The drag ended because the caller asked it to (`Tracker::cancel`), or could not go on.
    pub fn abort(&mut self, event: Event) -> Vec<Event> {
        if self.phase == Phase::Ended {
            return Vec::new();
        }
        self.phase = Phase::Ended;
        vec![event]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn machine() -> Machine {
        Machine::start().0
    }

    #[test]
    fn a_drag_starts_with_started() {
        assert_eq!(Machine::start().1, Event::Started);
    }

    #[test]
    fn a_completed_drop_reports_the_drop_then_finished() {
        let mut drag = machine();
        assert_eq!(
            drag.feed(Input::Target(Some("application/x-t".into()))),
            [Event::Target { accepted: true }]
        );
        assert_eq!(drag.feed(Input::DropPerformed), [Event::DropPerformed]);
        assert_eq!(drag.feed(Input::Finished), [Event::Finished]);
        assert!(drag.is_ended());
    }

    #[test]
    fn a_repeated_target_is_reported_once() {
        let mut drag = machine();
        let mime = || Input::Target(Some("application/x-t".into()));
        assert_eq!(drag.feed(mime()).len(), 1);
        assert!(drag.feed(mime()).is_empty());
        assert_eq!(
            drag.feed(Input::Target(None)),
            [Event::Target { accepted: false }]
        );
        assert!(drag.feed(Input::Target(None)).is_empty());
    }

    #[test]
    fn a_cancel_after_the_drop_is_a_drop_elsewhere() {
        let mut drag = machine();
        drag.feed(Input::DropPerformed);
        assert_eq!(
            drag.feed(Input::Cancelled),
            [Event::Cancelled { after_drop: true }]
        );
    }

    #[test]
    fn a_cancel_during_the_drag_snaps_back() {
        let mut drag = machine();
        assert_eq!(
            drag.feed(Input::Cancelled),
            [Event::Cancelled { after_drop: false }]
        );
    }

    #[test]
    fn a_cancel_right_after_finished_is_ignored() {
        let mut drag = machine();
        drag.feed(Input::DropPerformed);
        drag.feed(Input::Finished);
        assert!(drag.feed(Input::Cancelled).is_empty());
        assert!(drag.feed(Input::Finished).is_empty());
    }

    #[test]
    fn finished_after_a_cancel_is_ignored() {
        let mut drag = machine();
        drag.feed(Input::Cancelled);
        assert!(drag.feed(Input::Finished).is_empty());
        assert!(drag.feed(Input::DropPerformed).is_empty());
    }

    #[test]
    fn finished_without_a_drop_event_still_reports_the_drop() {
        let mut drag = machine();
        assert_eq!(
            drag.feed(Input::Finished),
            [Event::DropPerformed, Event::Finished]
        );
    }

    #[test]
    fn a_target_after_the_drop_is_ignored() {
        let mut drag = machine();
        drag.feed(Input::DropPerformed);
        assert!(drag.feed(Input::Target(None)).is_empty());
    }

    #[test]
    fn an_abort_ends_once() {
        let mut drag = machine();
        assert_eq!(
            drag.abort(Event::Cancelled { after_drop: false }),
            [Event::Cancelled { after_drop: false }]
        );
        assert!(drag.abort(Event::Failed("late".into())).is_empty());
        assert!(drag.feed(Input::Cancelled).is_empty());
    }
}
