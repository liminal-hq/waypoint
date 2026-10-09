// Builds the security descriptor text that lets only one user's account reach a pipe
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// True when `sid` is a SID in string form (`S-1-5-21-...`): the revision, then at least one more number, all digits separated by hyphens. Nothing else may reach the descriptor text, so that a hostile string cannot add an entry to the access list.
pub fn is_sid_string(sid: &str) -> bool {
    let Some(rest) = sid.strip_prefix("S-1-") else {
        return false;
    };
    !rest.is_empty()
        && rest.len() <= 200
        && rest
            .split('-')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}

/// The SDDL of a protected DACL with one entry: read and write for `sid`, and nothing for anyone else (not Everyone, not the anonymous account, not the network). The DACL is protected, so it takes nothing from a parent. `None` when `sid` is not a plain SID string.
pub fn pipe_dacl(sid: &str) -> Option<String> {
    is_sid_string(sid).then(|| format!("D:P(A;;GRGW;;;{sid})"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_user_sid_gets_one_protected_entry() {
        assert_eq!(
            pipe_dacl("S-1-5-21-1004336348-1177238915-682003330-1000").as_deref(),
            Some("D:P(A;;GRGW;;;S-1-5-21-1004336348-1177238915-682003330-1000)")
        );
    }

    #[test]
    fn nothing_but_a_plain_sid_reaches_the_descriptor() {
        for sid in [
            "",
            "S-1-",
            "S-1-5-21-",
            "S-1--5",
            "S-2-5-1",
            "WD",
            "S-1-1-0)(A;;GA;;;WD",
            "S-1-5-21-1000 ",
            "s-1-5-21-1",
            "S-1-5-a",
            "S-1-5-21-1000;",
        ] {
            assert!(pipe_dacl(sid).is_none(), "accepted {sid:?}");
        }
    }

    #[test]
    fn well_known_sids_are_plain_sids() {
        assert!(is_sid_string("S-1-5-18"));
        assert!(is_sid_string("S-1-1-0"));
    }
}
