// Certificate trust against a server that presents a certificate nobody trusts: the error says why
// and shows the certificate, nothing connects silently, and an answer holds for one connection only.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use std::sync::Arc;

use rcgen::{CertificateParams, DnType, KeyPair, SanType};
use rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
use support::config;
use support::mock::{multistatus, Mock, Reply};
use waypoint_path::VfsPath;
use waypoint_protocol::{ConnectionState, VfsError};
use waypoint_provider_webdav::WebDavProvider;
use waypoint_vfs::{CancelToken, ConnectAnswer, Provider};

struct Identity {
    config: Arc<rustls::ServerConfig>,
    fingerprint: String,
}

fn identity(common_name: &str) -> Identity {
    let mut params = CertificateParams::new(vec!["localhost".to_owned()]).unwrap();
    params
        .subject_alt_names
        .push(SanType::IpAddress("127.0.0.1".parse().unwrap()));
    params
        .distinguished_name
        .push(DnType::CommonName, common_name);
    let key = KeyPair::generate().unwrap();
    let cert = params.self_signed(&key).unwrap();
    let der = cert.der().clone();
    let fingerprint = ring::digest::digest(&ring::digest::SHA256, der.as_ref())
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(":");
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![der],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.serialize_der())),
        )
        .unwrap();
    Identity {
        config: Arc::new(config),
        fingerprint,
    }
}

fn secure(identity: &Identity) -> Mock {
    Mock::start_tls(identity.config.clone(), |seen, _| {
        let folder = seen.target.trim_end_matches('/');
        Reply::xml(
            207,
            multistatus(&format!("{folder}/"), (0..3).map(|n| format!("f{n}"))),
        )
    })
}

fn at(mock: &Mock) -> VfsPath {
    VfsPath::from_uri(&format!("davs://127.0.0.1:{}/x", mock.port)).unwrap()
}

fn provider() -> WebDavProvider {
    WebDavProvider::davs(config(None))
}

fn list(provider: &WebDavProvider, path: &VfsPath) -> Result<usize, VfsError> {
    provider
        .list(path, &CancelToken::new(), 0, &mut |_| {})
        .map(|entries| entries.len())
}

#[test]
fn an_untrusted_certificate_is_shown_and_nothing_is_sent() {
    let identity = identity("waypoint test server");
    let mock = secure(&identity);
    let provider = provider();
    let path = at(&mock);
    match list(&provider, &path) {
        Err(VfsError::CertificateUntrusted { certificate, .. }) => {
            assert_eq!(certificate.fingerprint, identity.fingerprint);
            assert_eq!(certificate.subject, "waypoint test server");
            assert_eq!(certificate.issuer, "waypoint test server");
            assert!(
                certificate.reason.contains("self-signed"),
                "{}",
                certificate.reason
            );
        }
        other => panic!("{other:?}"),
    }
    assert!(mock.requests().is_empty(), "the request was never made");
    assert!(matches!(
        provider.connection_state(&path.connection_key().unwrap()),
        ConnectionState::Failed {
            error: VfsError::CertificateUntrusted { .. }
        }
    ));
    // Asking again asks again: nothing was remembered.
    assert!(matches!(
        list(&provider, &path),
        Err(VfsError::CertificateUntrusted { .. })
    ));
}

#[test]
fn only_the_certificate_that_was_shown_is_trusted_by_an_answer() {
    let identity = identity("shown");
    let mock = secure(&identity);
    let provider = provider();
    let path = at(&mock);
    let key = path.connection_key().unwrap();
    // An answer naming some other certificate trusts nothing here.
    let wrong = provider.connect(
        &key,
        Some(ConnectAnswer::TrustCertificate {
            fingerprint: "00:11:22".to_owned(),
            remember: false,
        }),
        &CancelToken::new(),
    );
    match wrong {
        Err(VfsError::CertificateUntrusted { certificate, .. }) => {
            assert_eq!(certificate.fingerprint, identity.fingerprint)
        }
        other => panic!("{other:?}"),
    }
    assert!(mock.requests().is_empty());
    // The right one connects, and the connection works afterwards.
    provider
        .connect(
            &key,
            Some(ConnectAnswer::TrustCertificate {
                fingerprint: identity.fingerprint.clone(),
                remember: false,
            }),
            &CancelToken::new(),
        )
        .unwrap();
    assert_eq!(provider.connection_state(&key), ConnectionState::Connected);
    assert_eq!(list(&provider, &path).unwrap(), 3);
    assert!(!mock.requests().is_empty());
    // A provider that was not told knows nothing of it: the answer is for this connection only.
    assert!(matches!(
        list(&self::provider(), &path),
        Err(VfsError::CertificateUntrusted { .. })
    ));
}

#[test]
fn a_pin_from_the_saved_connection_is_trusted_without_a_question_for_that_connection_alone() {
    let identity = identity("pinned");
    let first = secure(&identity);
    let second = secure(&identity);
    let provider = provider();
    provider.pin_certificate(&at(&first).connection_key().unwrap(), &identity.fingerprint);
    assert_eq!(list(&provider, &at(&first)).unwrap(), 3);
    // The same certificate on another server (another connection) is not pinned there.
    assert!(matches!(
        list(&provider, &at(&second)),
        Err(VfsError::CertificateUntrusted { .. })
    ));
}

#[test]
fn a_changed_certificate_is_not_the_one_that_was_trusted() {
    let before = identity("before");
    let after = identity("after");
    let mock = secure(&before);
    let provider = provider();
    let key = at(&mock).connection_key().unwrap();
    provider.pin_certificate(&key, &after.fingerprint);
    match list(&provider, &at(&mock)) {
        Err(VfsError::CertificateUntrusted { certificate, .. }) => {
            assert_eq!(certificate.fingerprint, before.fingerprint);
            assert!(
                certificate.reason.contains("trusted before"),
                "{}",
                certificate.reason
            );
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_secure_connection_is_never_sent_to_an_insecure_address() {
    let identity = identity("redirector");
    let insecure = Mock::start(|_, _| Reply::new(207));
    let insecure_port = insecure.port;
    let mock = Mock::start_tls(identity.config.clone(), move |_, _| {
        Reply::new(307).header("Location", &format!("http://127.0.0.1:{insecure_port}/x"))
    });
    let provider = provider();
    provider.pin_certificate(&at(&mock).connection_key().unwrap(), &identity.fingerprint);
    assert!(matches!(
        list(&provider, &at(&mock)),
        Err(VfsError::Io { message, .. }) if message.contains("insecure")
    ));
    assert!(insecure.requests().is_empty());
}
