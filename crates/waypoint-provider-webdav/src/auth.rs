// HTTP authentication: reading a server's challenge, and answering it with Basic, Digest or a
// bearer token. Nothing here keeps a secret longer than the request it signs.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use md5::{Digest as _, Md5};
use ring::digest;
use ring::rand::{SecureRandom, SystemRandom};

/// How a server wants to be answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Challenge {
    Basic,
    Digest(DigestChallenge),
    Bearer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Algorithm {
    Md5,
    Sha256,
    Sha512_256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DigestChallenge {
    pub realm: String,
    pub nonce: String,
    pub opaque: Option<String>,
    pub algorithm: Algorithm,
    /// The `-sess` variant: the first hash covers the nonces too.
    pub session: bool,
    /// The server offers `qop=auth` (the one this client speaks; `auth-int` is not).
    pub qop_auth: bool,
    /// The nonce is old but the credential was fine: ask again with the new nonce.
    pub stale: bool,
}

/// Splits the values of every `WWW-Authenticate` header into challenges, in the order the server
/// gave them. A scheme this client does not speak is left out.
pub(crate) fn parse_challenges(values: &[&str]) -> Vec<Challenge> {
    let mut out = Vec::new();
    for value in values {
        for (scheme, params) in split_challenges(value) {
            match scheme.to_ascii_lowercase().as_str() {
                "basic" => out.push(Challenge::Basic),
                "bearer" => out.push(Challenge::Bearer),
                "digest" => {
                    if let Some(challenge) = digest_challenge(&params) {
                        out.push(Challenge::Digest(challenge));
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// The `scheme param=value, param="value"` groups of one header value. A comma followed by a token
/// without `=` begins a new challenge, since challenges and their parameters share commas.
fn split_challenges(value: &str) -> Vec<(String, Vec<(String, String)>)> {
    let mut groups: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for piece in split_commas(value) {
        let piece = piece.trim();
        if piece.is_empty() {
            continue;
        }
        let (head, rest) = match piece.split_once(char::is_whitespace) {
            Some((head, rest)) => (head, rest.trim()),
            None => (piece, ""),
        };
        if !head.contains('=') {
            // A scheme name, possibly with its first parameter after it.
            let mut params = Vec::new();
            if let Some(param) = parse_param(rest) {
                params.push(param);
            }
            groups.push((head.to_owned(), params));
        } else if let (Some(last), Some(param)) = (groups.last_mut(), parse_param(piece)) {
            last.1.push(param);
        }
    }
    groups
}

fn split_commas(value: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut start, mut quoted, mut escaped) = (0, false, false);
    for (at, c) in value.char_indices() {
        match c {
            _ if escaped => escaped = false,
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            ',' if !quoted => {
                out.push(&value[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }
    out.push(&value[start..]);
    out
}

fn parse_param(text: &str) -> Option<(String, String)> {
    let (name, value) = text.split_once('=')?;
    let value = value.trim();
    let value = match value.strip_prefix('"') {
        Some(quoted) => {
            let mut out = String::new();
            let mut escaped = false;
            for c in quoted.chars() {
                match c {
                    _ if escaped => {
                        out.push(c);
                        escaped = false;
                    }
                    '\\' => escaped = true,
                    '"' => break,
                    _ => out.push(c),
                }
            }
            out
        }
        None => value.to_owned(),
    };
    Some((name.trim().to_ascii_lowercase(), value))
}

fn digest_challenge(params: &[(String, String)]) -> Option<DigestChallenge> {
    let get = |name: &str| {
        params
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    };
    let algorithm = get("algorithm").unwrap_or("MD5").to_ascii_uppercase();
    let (algorithm, session) = match algorithm.as_str() {
        "MD5" => (Algorithm::Md5, false),
        "MD5-SESS" => (Algorithm::Md5, true),
        "SHA-256" => (Algorithm::Sha256, false),
        "SHA-256-SESS" => (Algorithm::Sha256, true),
        "SHA-512-256" => (Algorithm::Sha512_256, false),
        "SHA-512-256-SESS" => (Algorithm::Sha512_256, true),
        _ => return None,
    };
    Some(DigestChallenge {
        realm: get("realm")?.to_owned(),
        nonce: get("nonce")?.to_owned(),
        opaque: get("opaque").map(str::to_owned),
        algorithm,
        session,
        qop_auth: get("qop").is_some_and(|qop| {
            qop.split(',')
                .any(|option| option.trim().eq_ignore_ascii_case("auth"))
        }),
        stale: get("stale").is_some_and(|stale| stale.eq_ignore_ascii_case("true")),
    })
}

fn hash(algorithm: Algorithm, data: &[u8]) -> String {
    let bytes = match algorithm {
        Algorithm::Md5 => Md5::digest(data).to_vec(),
        Algorithm::Sha256 => digest::digest(&digest::SHA256, data).as_ref().to_vec(),
        Algorithm::Sha512_256 => digest::digest(&digest::SHA512_256, data).as_ref().to_vec(),
    };
    hex(&bytes)
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The `Authorization` header value of a digest answer, and the nonce count it used.
pub(crate) struct DigestState {
    pub challenge: DigestChallenge,
    count: u32,
}

impl DigestState {
    pub(crate) fn new(challenge: DigestChallenge) -> Self {
        Self {
            challenge,
            count: 0,
        }
    }

    /// Signs one request. `uri` is the request target as it is sent (path and query).
    pub(crate) fn authorize(
        &mut self,
        method: &str,
        uri: &str,
        user: &str,
        password: &[u8],
    ) -> String {
        self.count += 1;
        let mut raw = [0u8; 8];
        let _ = SystemRandom::new().fill(&mut raw);
        answer(
            &self.challenge,
            method,
            uri,
            user,
            password,
            self.count,
            &hex(&raw),
        )
    }
}

/// The answer to a challenge for one request (RFC 7616 §3.4). A function of its inputs, so the
/// RFC's own examples test it.
pub(crate) fn answer(
    challenge: &DigestChallenge,
    method: &str,
    uri: &str,
    user: &str,
    password: &[u8],
    count: u32,
    cnonce: &str,
) -> String {
    let algorithm = challenge.algorithm;
    let mut credential = format!("{user}:{}:", challenge.realm).into_bytes();
    credential.extend_from_slice(password);
    let mut ha1 = hash(algorithm, &credential);
    if challenge.session {
        ha1 = hash(
            algorithm,
            format!("{ha1}:{}:{cnonce}", challenge.nonce).as_bytes(),
        );
    }
    let ha2 = hash(algorithm, format!("{method}:{uri}").as_bytes());
    let nc = format!("{count:08x}");
    let response = if challenge.qop_auth {
        hash(
            algorithm,
            format!("{ha1}:{}:{nc}:{cnonce}:auth:{ha2}", challenge.nonce).as_bytes(),
        )
    } else {
        hash(
            algorithm,
            format!("{ha1}:{}:{ha2}", challenge.nonce).as_bytes(),
        )
    };
    let quote = |text: &str| text.replace('\\', "\\\\").replace('"', "\\\"");
    let mut header = format!(
        "Digest username=\"{}\", realm=\"{}\", nonce=\"{}\", uri=\"{}\", response=\"{response}\"",
        quote(user),
        quote(&challenge.realm),
        quote(&challenge.nonce),
        quote(uri),
    );
    header.push_str(match algorithm {
        Algorithm::Md5 if !challenge.session => "",
        Algorithm::Md5 => ", algorithm=MD5-sess",
        Algorithm::Sha256 if !challenge.session => ", algorithm=SHA-256",
        Algorithm::Sha256 => ", algorithm=SHA-256-sess",
        Algorithm::Sha512_256 if !challenge.session => ", algorithm=SHA-512-256",
        Algorithm::Sha512_256 => ", algorithm=SHA-512-256-sess",
    });
    if challenge.qop_auth {
        header.push_str(&format!(", qop=auth, nc={nc}, cnonce=\"{cnonce}\""));
    }
    if let Some(opaque) = &challenge.opaque {
        header.push_str(&format!(", opaque=\"{}\"", quote(opaque)));
    }
    header
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc_2617_s_example_signs_as_published() {
        let challenge = DigestChallenge {
            realm: "testrealm@host.com".into(),
            nonce: "dcd98b7102dd2f0e8b11d0f600bfb0c093".into(),
            opaque: Some("5ccc069c403ebaf9f0171e9517f40e41".into()),
            algorithm: Algorithm::Md5,
            session: false,
            qop_auth: true,
            stale: false,
        };
        let header = answer(
            &challenge,
            "GET",
            "/dir/index.html",
            "Mufasa",
            b"Circle Of Life",
            1,
            "0a4f113b",
        );
        assert!(
            header.contains("response=\"6629fae49393a05397450978507c4ef1\""),
            "{header}"
        );
        assert!(header.contains("nc=00000001") && header.contains("cnonce=\"0a4f113b\""));
        assert!(header.contains("opaque=\"5ccc069c403ebaf9f0171e9517f40e41\""));
    }

    #[test]
    fn rfc_7616_s_sha_256_example_signs_as_published() {
        let challenge = DigestChallenge {
            realm: "http-auth@example.org".into(),
            nonce: "7ypf/xlj9XXwfDPEoM4URrv/xwf94BcCAzFZH4GiTo0v".into(),
            opaque: Some("FQhe/qaU925kfnzjCev0ciny7QMkPqMAFRtzCUYo5tdS".into()),
            algorithm: Algorithm::Sha256,
            session: false,
            qop_auth: true,
            stale: false,
        };
        let header = answer(
            &challenge,
            "GET",
            "/dir/index.html",
            "Mufasa",
            b"Circle of Life",
            1,
            "f2/wE4q74E6zIJEtWaHKaf5wv/H5QzzpXusqGemxURZJ",
        );
        assert!(
            header.contains(
                "response=\"753927fa0e85d155564e2e272a28d1802ca10daf4496794697cf8db5856cb6c1\""
            ),
            "{header}"
        );
    }

    #[test]
    fn challenges_are_read_from_one_header_or_several() {
        let one = parse_challenges(&[
            r#"Basic realm="files", Digest realm="a, b", nonce="n1", qop="auth,auth-int", algorithm=SHA-256, stale=TRUE"#,
        ]);
        assert_eq!(one.len(), 2);
        assert_eq!(one[0], Challenge::Basic);
        let Challenge::Digest(digest) = &one[1] else {
            panic!("a digest challenge")
        };
        assert_eq!(digest.realm, "a, b");
        assert_eq!(digest.algorithm, Algorithm::Sha256);
        assert!(digest.qop_auth && digest.stale);
        let two = parse_challenges(&[
            "Bearer realm=\"x\"",
            "Negotiate",
            "Digest realm=\"r\", nonce=\"n\"",
        ]);
        assert_eq!(two.len(), 2);
        assert_eq!(two[0], Challenge::Bearer);
        // A digest this client cannot compute is not offered.
        assert!(parse_challenges(&[r#"Digest realm="r", nonce="n", algorithm=SHA-3"#]).is_empty());
    }

    #[test]
    fn nonce_counts_rise_with_every_request() {
        let challenge = digest_challenge(&[
            ("realm".into(), "r".into()),
            ("nonce".into(), "n".into()),
            ("qop".into(), "auth".into()),
        ])
        .unwrap();
        let mut state = DigestState::new(challenge);
        let first = state.authorize("GET", "/", "u", b"p");
        let second = state.authorize("GET", "/", "u", b"p");
        assert!(first.contains("nc=00000001") && second.contains("nc=00000002"));
    }
}
