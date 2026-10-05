// What an S3 failure means for the UI: each response and transport error becomes a typed `S3Error`,
// and from there the `VfsError` the page shows.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::error::Error as StdError;
use std::io::ErrorKind;

use aws_sdk_s3::config::http::HttpResponse;
use aws_sdk_s3::error::{ProvideErrorMetadata, SdkError};
use waypoint_protocol::{Location, UnreachableReason, VfsError};

/// A failure, classified. Everything the service or the network can say that Waypoint treats
/// differently is its own variant; the rest is `Service` with the service's own code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum S3Error {
    NoSuchBucket,
    NoSuchKey,
    /// The credential is valid and may not do this.
    AccessDenied,
    /// The access key id is unknown to the service.
    InvalidAccessKey,
    /// The secret does not match the key id (or the request was signed for another service).
    SignatureMismatch,
    /// A session token the service no longer accepts.
    ExpiredToken,
    /// This computer's clock and the service's differ by more than the service allows (15
    /// minutes on AWS), so every signed request is refused.
    ClockSkew,
    /// `SlowDown`, throttling or a 503: the service asked to wait. `retry_after_ms` is its
    /// `Retry-After`, if it sent one.
    Throttled {
        retry_after_ms: Option<u64>,
    },
    /// The bucket lives in another region; `region` is where, when the service said.
    Redirect {
        region: Option<String>,
    },
    /// The object is archived (Glacier Flexible Retrieval, Deep Archive) and needs a restore
    /// before it can be read. Waypoint never starts one.
    Archived,
    /// A conditional write found the name taken (`If-None-Match: *`).
    PreconditionFailed,
    /// The bucket is already there (`CreateBucket`).
    AlreadyExists,
    /// A range that starts past the end of the object.
    InvalidRange,
    StorageFull,
    /// The service does not implement the request (an S3-compatible store with gaps).
    NotImplemented,
    Unreachable(UnreachableReason),
    Timeout,
    /// The connection broke in the middle of a request.
    Disconnected,
    /// The service's TLS certificate is not trusted.
    Certificate,
    /// Anything else the service answered with.
    Service {
        status: Option<u16>,
        code: Option<String>,
    },
}

impl S3Error {
    /// The error the page shows for a failure at `location`.
    pub fn into_vfs(self, location: &Location) -> VfsError {
        let location = location.clone();
        match self {
            Self::NoSuchBucket | Self::NoSuchKey => VfsError::NotFound { location },
            Self::AccessDenied => VfsError::PermissionDenied { location },
            Self::InvalidAccessKey | Self::SignatureMismatch | Self::ExpiredToken => {
                VfsError::AuthFailed { location }
            }
            Self::ClockSkew => VfsError::Io {
                message: "the service refused the request because this computer's clock and the \
                          service's differ by more than 15 minutes; set the clock and try again"
                    .to_owned(),
                location: Some(location),
            },
            Self::Throttled { retry_after_ms } => VfsError::RateLimited {
                location,
                retry_after_ms,
            },
            Self::Redirect { region } => VfsError::Io {
                message: match region {
                    Some(region) => format!("the bucket is in another region ({region})"),
                    None => "the bucket is in another region".to_owned(),
                },
                location: Some(location),
            },
            Self::Archived => VfsError::Unsupported {
                what: "reading an archived object, which needs a restore first".to_owned(),
            },
            Self::PreconditionFailed | Self::AlreadyExists => VfsError::AlreadyExists { location },
            Self::InvalidRange => VfsError::Io {
                message: "the service refused the byte range".to_owned(),
                location: Some(location),
            },
            Self::StorageFull => VfsError::StorageFull { location },
            Self::NotImplemented => VfsError::Unsupported {
                what: "this request on this service".to_owned(),
            },
            Self::Unreachable(reason) => VfsError::Unreachable { location, reason },
            Self::Timeout => VfsError::Timeout { location },
            Self::Disconnected => VfsError::Disconnected { location },
            Self::Certificate => VfsError::Io {
                message: "the service's certificate is not trusted by this system".to_owned(),
                location: Some(location),
            },
            Self::Service { status, code } => VfsError::Io {
                message: match (code, status) {
                    (Some(code), _) => format!("the service refused the request ({code})"),
                    (None, Some(status)) => format!("the service answered {status}"),
                    (None, None) => "the service refused the request".to_owned(),
                },
                location: Some(location),
            },
        }
    }

    /// Whether the credential is what failed, so it must not be used again without asking.
    pub fn is_credential_failure(&self) -> bool {
        matches!(
            self,
            Self::InvalidAccessKey | Self::SignatureMismatch | Self::ExpiredToken
        )
    }

    /// Whether the connection is gone, so the next call starts from a fresh client.
    pub fn ends_session(&self) -> bool {
        matches!(
            self,
            Self::Unreachable(_) | Self::Timeout | Self::Disconnected
        )
    }
}

/// What a response said, before it is classified.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Response {
    pub status: Option<u16>,
    /// The `<Code>` of the error body (HEAD answers have none).
    pub code: Option<String>,
    pub message: Option<String>,
    /// The `Retry-After` header, in whole seconds.
    pub retry_after_secs: Option<u64>,
    /// The `x-amz-bucket-region` header.
    pub bucket_region: Option<String>,
    /// The `<Region>` of the error body.
    pub body_region: Option<String>,
}

/// Classifies a response by its status and the error code in its body.
pub fn classify_response(response: &Response) -> S3Error {
    let status = response.status;
    let code = response.code.as_deref();
    let region = || {
        response
            .bucket_region
            .clone()
            .or_else(|| response.body_region.clone())
            .or_else(|| response.message.as_deref().and_then(region_in_message))
    };
    match code {
        Some("NoSuchBucket") => return S3Error::NoSuchBucket,
        Some("NoSuchKey" | "NoSuchUpload") => return S3Error::NoSuchKey,
        Some("AccessDenied" | "AllAccessDisabled" | "AccountProblem") => {
            return S3Error::AccessDenied
        }
        Some("InvalidAccessKeyId") => return S3Error::InvalidAccessKey,
        Some("SignatureDoesNotMatch") => return S3Error::SignatureMismatch,
        Some("ExpiredToken" | "InvalidToken" | "TokenRefreshRequired") => {
            return S3Error::ExpiredToken
        }
        Some("RequestTimeTooSkewed" | "RequestExpired") => return S3Error::ClockSkew,
        Some(
            "PermanentRedirect" | "Redirect" | "TemporaryRedirect" | "AuthorizationHeaderMalformed",
        ) if status != Some(200) => {
            let region = region();
            // A malformed authorization with no region to retry with is the credential's fault.
            if region.is_some() || code != Some("AuthorizationHeaderMalformed") {
                return S3Error::Redirect { region };
            }
        }
        Some(
            "SlowDown"
            | "Throttling"
            | "ThrottlingException"
            | "RequestLimitExceeded"
            | "TooManyRequests"
            | "ServiceUnavailable"
            | "RequestThrottled"
            | "ReducedRate",
        ) => {
            return S3Error::Throttled {
                retry_after_ms: response.retry_after_secs.map(|secs| secs * 1000),
            }
        }
        Some("InvalidObjectState") => return S3Error::Archived,
        Some("PreconditionFailed" | "ConditionalRequestConflict") => {
            return S3Error::PreconditionFailed
        }
        Some("BucketAlreadyOwnedByYou" | "BucketAlreadyExists") => return S3Error::AlreadyExists,
        Some("InvalidRange") => return S3Error::InvalidRange,
        Some("QuotaExceeded" | "InsufficientStorage" | "StorageLimitExceeded") => {
            return S3Error::StorageFull
        }
        Some("NotImplemented") => return S3Error::NotImplemented,
        Some("NotFound") => return S3Error::NoSuchKey,
        _ => {}
    }
    match status {
        Some(301 | 307) if response.bucket_region.is_some() => S3Error::Redirect {
            region: response.bucket_region.clone(),
        },
        Some(404) => S3Error::NoSuchKey,
        Some(412) => S3Error::PreconditionFailed,
        Some(416) => S3Error::InvalidRange,
        Some(429 | 503) => S3Error::Throttled {
            retry_after_ms: response.retry_after_secs.map(|secs| secs * 1000),
        },
        Some(501) => S3Error::NotImplemented,
        Some(507) => S3Error::StorageFull,
        // A bare 403 (a HEAD has no body to say why) is the credential's lack of permission.
        Some(403) => S3Error::AccessDenied,
        Some(401) => S3Error::InvalidAccessKey,
        status => S3Error::Service {
            status,
            code: response.code.clone(),
        },
    }
}

/// The region of "the region 'us-east-1' is wrong; expecting 'eu-west-1'".
fn region_in_message(message: &str) -> Option<String> {
    let rest = message.split("expecting '").nth(1)?;
    let region = rest.split('\'').next()?;
    (!region.is_empty()).then(|| region.to_owned())
}

/// The text between `<tag>` and `</tag>` in a (tiny, flat) XML error body.
pub fn xml_text(body: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let start = body.find(&open)? + open.len();
    let end = body[start..].find(&format!("</{tag}>"))? + start;
    let text = &body[start..end];
    Some(
        text.replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
            .replace("&amp;", "&"),
    )
}

/// Classifies an SDK error: the response when there was one, else what went wrong on the wire.
pub(crate) fn from_sdk<E>(error: &SdkError<E, HttpResponse>) -> S3Error
where
    E: ProvideErrorMetadata + StdError + 'static,
{
    if let Some(raw) = error.raw_response() {
        let body = raw
            .body()
            .bytes()
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .unwrap_or("");
        let header = |name: &str| raw.headers().get(name).map(str::to_owned);
        let meta = error.meta();
        let response = Response {
            status: Some(raw.status().as_u16()),
            code: meta
                .code()
                .map(str::to_owned)
                .or_else(|| xml_text(body, "Code")),
            message: meta
                .message()
                .map(str::to_owned)
                .or_else(|| xml_text(body, "Message")),
            retry_after_secs: header("retry-after").and_then(|v| v.trim().parse().ok()),
            bucket_region: header("x-amz-bucket-region"),
            body_region: xml_text(body, "Region"),
        };
        log::debug!(
            "s3: {:?} {:?} {:?}",
            response.status,
            response.code,
            response.message
        );
        return classify_response(&response);
    }
    match error {
        SdkError::TimeoutError(_) => S3Error::Timeout,
        _ => transport(error),
    }
}

/// What the chain of causes under a dispatch failure says about the network.
fn transport(error: &(dyn StdError + 'static)) -> S3Error {
    let mut current: Option<&(dyn StdError + 'static)> = Some(error);
    let mut text = String::new();
    while let Some(error) = current {
        if let Some(io) = error.downcast_ref::<std::io::Error>() {
            match io.kind() {
                ErrorKind::ConnectionRefused => {
                    return S3Error::Unreachable(UnreachableReason::Refused)
                }
                ErrorKind::HostUnreachable | ErrorKind::NetworkUnreachable => {
                    return S3Error::Unreachable(UnreachableReason::NoRoute)
                }
                ErrorKind::NetworkDown => return S3Error::Unreachable(UnreachableReason::Offline),
                ErrorKind::TimedOut => return S3Error::Timeout,
                ErrorKind::ConnectionReset
                | ErrorKind::ConnectionAborted
                | ErrorKind::BrokenPipe
                | ErrorKind::UnexpectedEof => return S3Error::Disconnected,
                _ => {}
            }
        }
        text.push_str(&error.to_string().to_ascii_lowercase());
        text.push('\n');
        current = error.source();
    }
    if text.contains("dns error")
        || text.contains("failed to lookup address")
        || text.contains("name or service not known")
        || text.contains("no such host")
        || text.contains("nodename nor servname")
        || text.contains("temporary failure in name resolution")
    {
        S3Error::Unreachable(UnreachableReason::NameNotResolved)
    } else if text.contains("invalid peer certificate")
        || text.contains("unknownissuer")
        || text.contains("certificate verify failed")
        || text.contains("invalidcertificate")
    {
        S3Error::Certificate
    } else if text.contains("timed out") || text.contains("timeout") {
        S3Error::Timeout
    } else if text.contains("connection refused") {
        S3Error::Unreachable(UnreachableReason::Refused)
    } else if text.contains("connection closed")
        || text.contains("connection reset")
        || text.contains("broken pipe")
        || text.contains("incomplete message")
    {
        S3Error::Disconnected
    } else {
        S3Error::Service {
            status: None,
            code: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(status: u16, body: &str) -> S3Error {
        classify_response(&Response {
            status: Some(status),
            code: xml_text(body, "Code"),
            message: xml_text(body, "Message"),
            retry_after_secs: None,
            bucket_region: None,
            body_region: xml_text(body, "Region"),
        })
    }

    const NO_SUCH_KEY: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Error><Code>NoSuchKey</Code><Message>The specified key does not exist.</Message><Key>a.txt</Key><RequestId>4442587FB7D0A2F9</RequestId></Error>"#;
    const NO_SUCH_BUCKET: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Error><Code>NoSuchBucket</Code><Message>The specified bucket does not exist</Message><BucketName>nope</BucketName></Error>"#;
    const ACCESS_DENIED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Error><Code>AccessDenied</Code><Message>Access Denied</Message><RequestId>X</RequestId></Error>"#;
    const BAD_KEY: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Error><Code>InvalidAccessKeyId</Code><Message>The AWS Access Key Id you provided does not exist in our records.</Message><AWSAccessKeyId>AKIA</AWSAccessKeyId></Error>"#;
    const BAD_SIGNATURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Error><Code>SignatureDoesNotMatch</Code><Message>The request signature we calculated does not match the signature you provided.</Message></Error>"#;
    const SKEW: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Error><Code>RequestTimeTooSkewed</Code><Message>The difference between the request time and the current time is too large.</Message><MaxAllowedSkewMilliseconds>900000</MaxAllowedSkewMilliseconds></Error>"#;
    const SLOW_DOWN: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Error><Code>SlowDown</Code><Message>Please reduce your request rate.</Message></Error>"#;
    const WRONG_REGION: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Error><Code>AuthorizationHeaderMalformed</Code><Message>The authorization header is malformed; the region 'us-east-1' is wrong; expecting 'eu-west-1'</Message><Region>eu-west-1</Region></Error>"#;
    const REDIRECT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Error><Code>PermanentRedirect</Code><Message>The bucket you are attempting to access must be addressed using the specified endpoint.</Message><Endpoint>b.s3.eu-west-1.amazonaws.com</Endpoint></Error>"#;
    const ARCHIVED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Error><Code>InvalidObjectState</Code><Message>The operation is not valid for the object's storage class</Message><StorageClass>GLACIER</StorageClass></Error>"#;
    const PRECONDITION: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Error><Code>PreconditionFailed</Code><Message>At least one of the pre-conditions you specified did not hold</Message><Condition>If-None-Match</Condition></Error>"#;

    #[test]
    fn recorded_error_bodies_become_typed_errors() {
        assert_eq!(parse(404, NO_SUCH_KEY), S3Error::NoSuchKey);
        assert_eq!(parse(404, NO_SUCH_BUCKET), S3Error::NoSuchBucket);
        assert_eq!(parse(403, ACCESS_DENIED), S3Error::AccessDenied);
        assert_eq!(parse(403, BAD_KEY), S3Error::InvalidAccessKey);
        assert_eq!(parse(403, BAD_SIGNATURE), S3Error::SignatureMismatch);
        assert_eq!(parse(403, SKEW), S3Error::ClockSkew);
        assert_eq!(
            parse(503, SLOW_DOWN),
            S3Error::Throttled {
                retry_after_ms: None
            }
        );
        assert_eq!(
            parse(400, WRONG_REGION),
            S3Error::Redirect {
                region: Some("eu-west-1".to_owned())
            }
        );
        assert_eq!(parse(301, REDIRECT), S3Error::Redirect { region: None });
        assert_eq!(parse(403, ARCHIVED), S3Error::Archived);
        assert_eq!(parse(412, PRECONDITION), S3Error::PreconditionFailed);
    }

    #[test]
    fn bodies_without_a_code_fall_back_to_the_status() {
        let status = |status| {
            classify_response(&Response {
                status: Some(status),
                ..Response::default()
            })
        };
        assert_eq!(status(404), S3Error::NoSuchKey);
        assert_eq!(status(403), S3Error::AccessDenied);
        assert_eq!(status(416), S3Error::InvalidRange);
        assert_eq!(status(501), S3Error::NotImplemented);
        assert_eq!(
            status(500),
            S3Error::Service {
                status: Some(500),
                code: None
            }
        );
    }

    #[test]
    fn throttling_carries_the_retry_after_header_and_a_redirect_the_region_header() {
        let throttled = classify_response(&Response {
            status: Some(503),
            code: Some("SlowDown".to_owned()),
            retry_after_secs: Some(7),
            ..Response::default()
        });
        assert_eq!(
            throttled,
            S3Error::Throttled {
                retry_after_ms: Some(7000)
            }
        );
        let redirect = classify_response(&Response {
            status: Some(301),
            code: Some("PermanentRedirect".to_owned()),
            bucket_region: Some("ap-southeast-2".to_owned()),
            ..Response::default()
        });
        assert_eq!(
            redirect,
            S3Error::Redirect {
                region: Some("ap-southeast-2".to_owned())
            }
        );
        // A HEAD has no body, only the header.
        let head = classify_response(&Response {
            status: Some(301),
            bucket_region: Some("eu-north-1".to_owned()),
            ..Response::default()
        });
        assert!(matches!(head, S3Error::Redirect { region: Some(_) }));
    }

    #[test]
    fn a_failure_is_worded_for_the_page_without_the_services_own_text() {
        let here = Location::new("s3://b/k", "s3://b/k");
        assert!(matches!(
            S3Error::NoSuchKey.into_vfs(&here),
            VfsError::NotFound { .. }
        ));
        assert!(matches!(
            S3Error::SignatureMismatch.into_vfs(&here),
            VfsError::AuthFailed { .. }
        ));
        assert!(matches!(
            S3Error::Archived.into_vfs(&here),
            VfsError::Unsupported { .. }
        ));
        let VfsError::Io { message, .. } = S3Error::ClockSkew.into_vfs(&here) else {
            panic!("clock skew is an I/O error with its own wording");
        };
        assert!(message.contains("clock"));
        assert!(S3Error::InvalidAccessKey.is_credential_failure());
        assert!(!S3Error::AccessDenied.is_credential_failure());
        assert!(S3Error::Timeout.ends_session());
    }

    #[test]
    fn xml_text_unescapes_entities() {
        assert_eq!(
            xml_text("<A><B>a &amp; b &lt;c&gt;</B></A>", "B").as_deref(),
            Some("a & b <c>")
        );
        assert_eq!(xml_text("<A></A>", "B"), None);
    }

    #[test]
    fn the_wire_says_why_nothing_answered() {
        let refused = std::io::Error::from(ErrorKind::ConnectionRefused);
        assert_eq!(
            transport(&refused),
            S3Error::Unreachable(UnreachableReason::Refused)
        );
        let dns = std::io::Error::other("dns error: failed to lookup address information");
        assert_eq!(
            transport(&dns),
            S3Error::Unreachable(UnreachableReason::NameNotResolved)
        );
        let tls = std::io::Error::other("invalid peer certificate: UnknownIssuer");
        assert_eq!(transport(&tls), S3Error::Certificate);
        let reset = std::io::Error::from(ErrorKind::ConnectionReset);
        assert_eq!(transport(&reset), S3Error::Disconnected);
    }
}
