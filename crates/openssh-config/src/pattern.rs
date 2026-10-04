// OpenSSH's host patterns: `*` and `?` wildcards, and comma or space separated lists with `!`
// negation.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Whether `text` matches `pattern`, where `*` matches any run of characters and `?` any one.
/// Host names compare without regard to ASCII case, as OpenSSH's do.
pub(crate) fn wildcard(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.to_ascii_lowercase().chars().collect();
    let text: Vec<char> = text.to_ascii_lowercase().chars().collect();
    let (mut p, mut t) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while t < text.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == text[t]) {
            p += 1;
            t += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some((p, t));
            p += 1;
        } else if let Some((sp, st)) = star {
            p = sp + 1;
            t = st + 1;
            star = Some((sp, st + 1));
        } else {
            return false;
        }
    }
    pattern[p..].iter().all(|&c| c == '*')
}

/// Whether `host` matches a list of patterns: no negated pattern may match, and at least one
/// plain pattern must.
pub(crate) fn list_matches<'a>(patterns: impl IntoIterator<Item = &'a str>, host: &str) -> bool {
    let mut matched = false;
    for pattern in patterns {
        if let Some(negated) = pattern.strip_prefix('!') {
            if wildcard(negated, host) {
                return false;
            }
        } else if wildcard(pattern, host) {
            matched = true;
        }
    }
    matched
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcards_match_as_openssh_does() {
        assert!(wildcard("*", "anything"));
        assert!(wildcard("*.lan", "nas.lan"));
        assert!(wildcard("NAS.lan", "nas.LAN"));
        assert!(wildcard("n?s", "nas"));
        assert!(!wildcard("n?s", "nads"));
        assert!(wildcard("[nas]:22*", "[nas]:2222"));
        assert!(!wildcard("*.lan", "nas.lan.example"));
        assert!(wildcard("a*b*c", "aXXbYYc"));
    }

    #[test]
    fn a_negated_pattern_wins() {
        assert!(list_matches(["*.lan", "!secret.lan"], "nas.lan"));
        assert!(!list_matches(["*.lan", "!secret.lan"], "secret.lan"));
        assert!(!list_matches(["!secret.lan"], "nas.lan"));
    }
}
