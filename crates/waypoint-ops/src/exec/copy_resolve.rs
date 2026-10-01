// What the user has decided about conflicts, and what each decision means for one clash (A48).
//
// Nothing is overwritten without a decision and there is no default policy: `Resolutions::default`
// is empty, `policy_for` answers `None` for a clash nobody has spoken to, and the engine asks.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;

use waypoint_protocol::Location;

use crate::model::{Conflict, ConflictKind, ConflictPolicy, OpsError, Resolution};

/// The answers a job has been given, kept on the job so a retried or resumed run does not ask
/// again.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Resolutions {
    /// The policy for every conflict not answered one by one (the "apply to all" answer, or the
    /// one the request carried).
    all: Option<ConflictPolicy>,
    /// Answers for one source's clash, by the source's URI.
    by_source: HashMap<String, ConflictPolicy>,
}

impl Resolutions {
    /// Starts with the policy a request carried, if it carried one.
    pub fn new(all: Option<ConflictPolicy>) -> Self {
        Self {
            all,
            by_source: HashMap::new(),
        }
    }

    /// The policy applied to every clash without an answer of its own.
    pub fn all(&self) -> Option<ConflictPolicy> {
        self.all
    }

    /// Records an answer. One without a source is the "apply to all" answer and is kept as the
    /// job's current policy; one with a source applies to that source's clash only.
    pub fn apply(&mut self, resolution: &Resolution) {
        match &resolution.source {
            None => self.all = Some(resolution.policy),
            Some(source) => {
                self.by_source.insert(source.uri.clone(), resolution.policy);
            }
        }
    }

    /// Records an answer for one source's clash.
    pub fn set_for(&mut self, source: &Location, policy: ConflictPolicy) {
        self.by_source.insert(source.uri.clone(), policy);
    }

    /// Records a policy for every remaining clash.
    pub fn set_all(&mut self, policy: ConflictPolicy) {
        self.all = Some(policy);
    }

    /// The decision that covers a clash of `source`, if there is one: the source's own answer first,
    /// then the policy for all.
    pub fn policy_for(&self, source: &Location) -> Option<ConflictPolicy> {
        self.by_source.get(&source.uri).copied().or(self.all)
    }

    /// The conflicts nobody has decided yet, in order.
    pub fn unresolved(&self, conflicts: &[Conflict]) -> Vec<Conflict> {
        conflicts
            .iter()
            .filter(|c| self.policy_for(&c.source).is_none())
            .cloned()
            .collect()
    }
}

/// What a policy comes to for one clash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Leave the entry out.
    Skip,
    /// Put the new entry under a free name.
    KeepBoth,
    /// Swap the new entry in for the existing one (the old one goes aside until the new one is in
    /// place).
    Replace,
    /// Fold the folder into the existing folder; its clashes are decided one by one.
    Merge,
}

/// What `policy` means for a clash of `kind`. `Ok(None)` means the choice cannot decide this clash
/// (`MergeFolders` for a file, say) and the engine asks again; `Err` is a choice that must not be
/// carried out. `source_newer` is `Some(true)` when the source's modification time is later than
/// the existing entry's, `Some(false)` when it is not, and `None` when either is unknown.
pub fn action_for(
    policy: ConflictPolicy,
    kind: ConflictKind,
    source_newer: Option<bool>,
    existing: &Location,
) -> Result<Option<Action>, OpsError> {
    use ConflictKind::*;
    use ConflictPolicy::*;
    Ok(match (policy, kind) {
        (Skip, _) => Some(Action::Skip),
        (KeepBoth, _) => Some(Action::KeepBoth),
        (Replace, FileOverFile | FolderOverFolder) => Some(Action::Replace),
        // A file and a folder sharing a name: replacing either would delete a tree or hide a file
        // behind a folder, so only Skip and Keep both can settle it.
        (Replace, FileOverFolder | FolderOverFile) => {
            return Err(OpsError::CannotReplace {
                location: existing.clone(),
            })
        }
        (MergeFolders | ReplaceIfNewer, FolderOverFolder) => Some(Action::Merge),
        (MergeFolders, _) => None,
        // Only a clearly newer source replaces; equal or unknown times leave the existing entry.
        (ReplaceIfNewer, FileOverFile) => Some(if source_newer == Some(true) {
            Action::Replace
        } else {
            Action::Skip
        }),
        (ReplaceIfNewer, _) => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loc(name: &str) -> Location {
        Location::new(format!("/{name}"), format!("file:///{name}"))
    }

    fn conflict(name: &str) -> Conflict {
        Conflict {
            source: loc(name),
            existing: loc(&format!("d/{name}")),
            name: name.to_owned(),
            kind: ConflictKind::FileOverFile,
            within_batch: false,
            source_size: None,
            existing_size: None,
            source_modified_ms: None,
            existing_modified_ms: None,
        }
    }

    #[test]
    fn nothing_is_decided_by_default() {
        let r = Resolutions::default();
        assert_eq!(r.policy_for(&loc("a")), None);
        assert_eq!(r.unresolved(&[conflict("a")]).len(), 1);
    }

    #[test]
    fn an_answer_for_one_source_covers_only_that_source() {
        let mut r = Resolutions::default();
        r.apply(&Resolution {
            source: Some(loc("a")),
            policy: ConflictPolicy::Skip,
        });
        assert_eq!(r.policy_for(&loc("a")), Some(ConflictPolicy::Skip));
        assert_eq!(r.policy_for(&loc("b")), None);
        let left = r.unresolved(&[conflict("a"), conflict("b")]);
        assert_eq!(left, vec![conflict("b")]);
    }

    #[test]
    fn an_answer_for_all_covers_the_rest_and_a_specific_answer_beats_it() {
        let mut r = Resolutions::default();
        r.apply(&Resolution {
            source: None,
            policy: ConflictPolicy::Replace,
        });
        r.apply(&Resolution {
            source: Some(loc("a")),
            policy: ConflictPolicy::Skip,
        });
        assert_eq!(r.all(), Some(ConflictPolicy::Replace));
        assert_eq!(r.policy_for(&loc("a")), Some(ConflictPolicy::Skip));
        assert_eq!(r.policy_for(&loc("z")), Some(ConflictPolicy::Replace));
        assert!(r.unresolved(&[conflict("a"), conflict("z")]).is_empty());
    }

    #[test]
    fn a_request_can_carry_the_policy_for_all() {
        let r = Resolutions::new(Some(ConflictPolicy::KeepBoth));
        assert_eq!(r.policy_for(&loc("x")), Some(ConflictPolicy::KeepBoth));
    }

    #[test]
    fn the_policy_table() {
        use ConflictKind::*;
        use ConflictPolicy::*;
        let existing = loc("e");
        let act = |p, k, newer| action_for(p, k, newer, &existing);
        // Skip and Keep both settle anything.
        for kind in [
            FileOverFile,
            FolderOverFolder,
            FileOverFolder,
            FolderOverFile,
        ] {
            assert_eq!(act(Skip, kind, None), Ok(Some(Action::Skip)));
            assert_eq!(act(KeepBoth, kind, None), Ok(Some(Action::KeepBoth)));
        }
        // Replace works for like with like and refuses a file against a folder.
        assert_eq!(act(Replace, FileOverFile, None), Ok(Some(Action::Replace)));
        assert_eq!(
            act(Replace, FolderOverFolder, None),
            Ok(Some(Action::Replace))
        );
        for kind in [FileOverFolder, FolderOverFile] {
            assert_eq!(
                act(Replace, kind, None),
                Err(OpsError::CannotReplace {
                    location: existing.clone()
                })
            );
        }
        // Merge and Replace if newer merge folders and cannot settle anything else.
        for policy in [MergeFolders, ReplaceIfNewer] {
            assert_eq!(act(policy, FolderOverFolder, None), Ok(Some(Action::Merge)));
            assert_eq!(act(policy, FileOverFolder, None), Ok(None));
            assert_eq!(act(policy, FolderOverFile, None), Ok(None));
        }
        assert_eq!(act(MergeFolders, FileOverFile, None), Ok(None));
    }

    #[test]
    fn replace_if_newer_replaces_only_a_clearly_newer_source() {
        use ConflictKind::FileOverFile;
        let existing = loc("e");
        let act = |newer| {
            action_for(
                ConflictPolicy::ReplaceIfNewer,
                FileOverFile,
                newer,
                &existing,
            )
        };
        assert_eq!(act(Some(true)), Ok(Some(Action::Replace)));
        // Older, equal (reported as not newer) or unknown all leave the existing file.
        assert_eq!(act(Some(false)), Ok(Some(Action::Skip)));
        assert_eq!(act(None), Ok(Some(Action::Skip)));
    }
}
