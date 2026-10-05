// Certificate trust: the system's verdict first, then the fingerprints a person has trusted for this
// connection, and a record of the certificate that was turned away so the error can show it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashSet;
use std::sync::{Arc, Mutex, MutexGuard};

use ring::digest;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{CertificateError, DigitallySignedStruct, Error, SignatureScheme};
use waypoint_protocol::Certificate;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// A certificate the system did not trust, kept until the error that reports it is built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Rejected {
    pub der: Vec<u8>,
    pub reason: String,
}

/// What one connection trusts beyond the system: certificates by fingerprint, for as long as the
/// provider lives (an answer to a question, or a pin the saved connection supplied). It is never
/// written anywhere and never added to the system's store.
#[derive(Debug, Default)]
pub(crate) struct TrustState {
    trusted: Mutex<HashSet<String>>,
    rejected: Mutex<Option<Rejected>>,
}

impl TrustState {
    pub(crate) fn trust(&self, fingerprint: &str) {
        lock(&self.trusted).insert(normalise(fingerprint));
    }

    fn is_trusted(&self, fingerprint: &str) -> bool {
        lock(&self.trusted).contains(&normalise(fingerprint))
    }

    pub(crate) fn has_trusted(&self) -> bool {
        !lock(&self.trusted).is_empty()
    }

    /// The certificate turned away since the last call, if any.
    pub(crate) fn take_rejected(&self) -> Option<Rejected> {
        lock(&self.rejected).take()
    }

    pub(crate) fn clear_rejected(&self) {
        *lock(&self.rejected) = None;
    }
}

/// Fingerprints compare without regard to case, separators or a `SHA256:` label.
fn normalise(fingerprint: &str) -> String {
    let text = fingerprint
        .trim()
        .trim_start_matches("SHA256:")
        .trim_start_matches("sha256:");
    text.chars()
        .filter(|c| c.is_ascii_hexdigit())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// The SHA-256 digest of a certificate in colon-separated upper-case hex, as the trust dialog
/// shows it.
pub(crate) fn fingerprint(der: &[u8]) -> String {
    digest::digest(&digest::SHA256, der)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}

/// What the dialog shows of a certificate: subject, issuer, fingerprint and why it was refused.
pub(crate) fn describe(rejected: &Rejected, previously_trusted: bool) -> Certificate {
    let (subject, issuer) = match x509_parser::parse_x509_certificate(&rejected.der) {
        Ok((_, cert)) => (name_of(cert.subject()), name_of(cert.issuer())),
        Err(_) => ("an unreadable certificate".to_owned(), String::new()),
    };
    let mut reason = rejected.reason.clone();
    if previously_trusted {
        reason.push_str(" (it is not the certificate you trusted before)");
    }
    Certificate {
        subject,
        issuer,
        fingerprint: fingerprint(&rejected.der),
        reason,
    }
}

fn name_of(name: &x509_parser::x509::X509Name<'_>) -> String {
    name.iter_common_name()
        .next()
        .and_then(|cn| cn.as_str().ok())
        .map(str::to_owned)
        .unwrap_or_else(|| name.to_string())
}

fn reason(error: &CertificateError) -> String {
    match error {
        CertificateError::UnknownIssuer => {
            "it was not issued by an authority this computer trusts (it may be self-signed)"
                .to_owned()
        }
        CertificateError::Expired | CertificateError::ExpiredContext { .. } => {
            "it has expired".to_owned()
        }
        CertificateError::NotValidYet | CertificateError::NotValidYetContext { .. } => {
            "it is not valid yet".to_owned()
        }
        CertificateError::NotValidForName | CertificateError::NotValidForNameContext { .. } => {
            "it was issued for another name than the server's address".to_owned()
        }
        CertificateError::Revoked => "it has been revoked".to_owned(),
        other => format!("this computer could not verify it ({other:?})"),
    }
}

/// Asks the system's verifier, and accepts what it refuses only for a certificate whose
/// fingerprint was trusted. A refusal is recorded, so the request's error can say which
/// certificate it was.
#[derive(Debug)]
pub(crate) struct TrustVerifier {
    pub inner: Arc<dyn ServerCertVerifier>,
    pub trust: Arc<TrustState>,
}

impl ServerCertVerifier for TrustVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        let verdict = self.inner.verify_server_cert(
            end_entity,
            intermediates,
            server_name,
            ocsp_response,
            now,
        );
        let why = match &verdict {
            Ok(_) => return verdict,
            Err(Error::InvalidCertificate(error)) => reason(error),
            Err(Error::General(message)) => {
                format!("this computer could not verify it ({message})")
            }
            Err(_) => return verdict,
        };
        if self.trust.is_trusted(&fingerprint(end_entity.as_ref())) {
            return Ok(ServerCertVerified::assertion());
        }
        *lock(&self.trust.rejected) = Some(Rejected {
            der: end_entity.as_ref().to_vec(),
            reason: why,
        });
        verdict
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.inner.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.inner.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.inner.supported_verify_schemes()
    }
}

/// A TLS configuration that trusts the system's roots (and the enterprise roots the platform
/// verifier knows) plus whatever `trust` holds.
pub(crate) fn client_config(trust: Arc<TrustState>) -> Result<rustls::ClientConfig, String> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let inner = rustls_platform_verifier::Verifier::new(provider.clone())
        .map_err(|error| format!("the system's certificate store could not be read: {error}"))?;
    let verifier = TrustVerifier {
        inner: Arc::new(inner),
        trust,
    };
    let mut config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|error| error.to_string())?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprints_compare_however_they_are_written() {
        let trust = TrustState::default();
        trust.trust("aa:BB:01");
        assert!(trust.is_trusted("AA:BB:01"));
        assert!(trust.is_trusted("sha256:aabb01"));
        assert!(!trust.is_trusted("AA:BB:02"));
        assert!(trust.has_trusted());
    }

    #[test]
    fn a_fingerprint_is_the_sha_256_in_colon_hex() {
        let print = fingerprint(b"abc");
        assert_eq!(
            print,
            "BA:78:16:BF:8F:01:CF:EA:41:41:40:DE:5D:AE:22:23:B0:03:61:A3:96:17:7A:9C:B4:10:FF:61:F2:00:15:AD"
        );
    }
}
