// One connection's HTTP session: the client, what has been learned about how the server wants
// callers to log in, and the one place a request is sent, answered when challenged, redirected and
// turned into a typed error.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, LOCATION, RETRY_AFTER};
use reqwest::{Method, StatusCode, Url};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use waypoint_path::ConnectionKey;
use waypoint_protocol::{AuthPrompt, Location, VfsError};
use waypoint_vfs::{CancelToken, Credential, CredentialSource, Secret};

use crate::auth::{parse_challenges, Challenge, DigestState};
use crate::errors::{from_transport, retry_after};
use crate::options::{AuthMode, WebDavOptions};
use crate::spool::Upload;
use crate::tls::{client_config, describe, TrustState};

/// Redirects followed before giving up.
const MAX_REDIRECTS: usize = 5;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// What a request carries.
pub(crate) enum Payload {
    None,
    Bytes(Vec<u8>),
    Upload(Arc<Upload>),
}

/// A request, as data, so it can be sent again after a challenge or a redirect.
pub(crate) struct Request {
    pub method: &'static str,
    pub url: String,
    pub headers: Vec<(&'static str, String)>,
    pub payload: Payload,
}

impl Request {
    pub(crate) fn new(method: &'static str, url: String) -> Self {
        Self {
            method,
            url,
            headers: Vec::new(),
            payload: Payload::None,
        }
    }

    pub(crate) fn header(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.headers.push((name, value.into()));
        self
    }

    pub(crate) fn bytes(mut self, bytes: Vec<u8>) -> Self {
        self.payload = Payload::Bytes(bytes);
        self
    }

    pub(crate) fn upload(mut self, upload: Arc<Upload>) -> Self {
        self.payload = Payload::Upload(upload);
        self
    }

    /// Whether sending it twice does no harm, so a dropped connection may be retried by itself.
    pub(crate) fn idempotent(&self) -> bool {
        matches!(self.method, "GET" | "HEAD" | "PROPFIND" | "OPTIONS")
    }
}

/// A response whose headers have arrived. Holding it holds the connection's place in the queue.
pub(crate) struct Reply {
    pub(crate) response: reqwest::Response,
    pub(crate) _permit: OwnedSemaphorePermit,
}

impl Reply {
    pub(crate) fn status(&self) -> StatusCode {
        self.response.status()
    }

    pub(crate) fn header(&self, name: &str) -> Option<&str> {
        self.response.headers().get(name)?.to_str().ok()
    }

    /// Lets go of the queue's place, keeping the response.
    pub(crate) fn into_response(self) -> reqwest::Response {
        self.response
    }
}

#[derive(Debug, Clone)]
struct Cred {
    user: Option<String>,
    secret: Secret,
}

enum Scheme {
    Basic,
    Bearer,
    Digest(DigestState),
}

#[derive(Default)]
struct AuthState {
    cred: Option<Cred>,
    scheme: Option<Scheme>,
}

/// One connection: a client with its own connection pool, its login state and its queue.
pub(crate) struct Session {
    client: reqwest::Client,
    pub(crate) trust: Arc<TrustState>,
    auth: Mutex<AuthState>,
    gate: Arc<Semaphore>,
    pub(crate) options: WebDavOptions,
    credentials: Arc<dyn CredentialSource>,
    key: ConnectionKey,
    /// The user named in the location, which a login with none of its own is made as.
    user: Option<String>,
    /// The origin a login is only ever sent to.
    origin: String,
}

impl Session {
    pub(crate) fn new(
        key: ConnectionKey,
        user: Option<String>,
        origin: String,
        options: WebDavOptions,
        credentials: Arc<dyn CredentialSource>,
        trust: Arc<TrustState>,
    ) -> Result<Self, VfsError> {
        let origin =
            Url::parse(&origin).map_err(|_| VfsError::InvalidLocation { input: origin })?;
        let origin = origin_of(&origin);
        let tls = client_config(trust.clone()).map_err(|message| VfsError::Io {
            message,
            location: None,
        })?;
        let mut builder = reqwest::Client::builder()
            .use_preconfigured_tls(tls)
            .http1_only()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(options.timeout)
            .read_timeout(options.timeout)
            .pool_idle_timeout(Duration::from_secs(60))
            .tcp_keepalive(Duration::from_secs(30))
            .user_agent(concat!("Waypoint/", env!("CARGO_PKG_VERSION")));
        if !options.system_proxy {
            builder = builder.no_proxy();
        }
        let client = builder.build().map_err(|error| VfsError::Io {
            message: format!("the HTTP client could not start: {error}"),
            location: None,
        })?;
        Ok(Self {
            client,
            trust,
            auth: Mutex::new(AuthState::default()),
            gate: Arc::new(Semaphore::new(options.max_requests.max(1))),
            options,
            credentials,
            key,
            user,
            origin,
        })
    }

    /// Takes the person's answer to a login question.
    pub(crate) fn answer(&self, credential: Credential) -> bool {
        let cred = match credential {
            Credential::Password { user, password } => Cred {
                user,
                secret: password,
            },
            Credential::Passphrase(token) => Cred {
                user: None,
                secret: token,
            },
            _ => return false,
        };
        lock(&self.auth).cred = Some(cred);
        true
    }

    fn prompt(&self, bearer: bool) -> AuthPrompt {
        if bearer {
            AuthPrompt::Passphrase {
                subject: "access token".to_owned(),
            }
        } else {
            AuthPrompt::Password {
                user: self.user.clone(),
            }
        }
    }

    /// Makes sure a credential for the scheme is at hand: the one held, or the source's. Without
    /// one the call fails with what to ask for.
    fn ensure_credential(&self, bearer: bool, location: &Location) -> Result<(), VfsError> {
        let mut auth = lock(&self.auth);
        let usable = |cred: &Cred| bearer || cred.user.is_some() || self.user.is_some();
        if auth.cred.as_ref().is_some_and(usable) {
            return Ok(());
        }
        let prompt = self.prompt(bearer);
        let found = match (self.credentials.credential(&self.key, &prompt), bearer) {
            (Some(Credential::Password { user, password }), false) => Some(Cred {
                user,
                secret: password,
            }),
            (Some(Credential::Passphrase(token)), true) => Some(Cred {
                user: None,
                secret: token,
            }),
            _ => None,
        };
        match found.filter(usable) {
            Some(cred) => {
                auth.cred = Some(cred);
                Ok(())
            }
            None => Err(VfsError::AuthRequired {
                location: location.clone(),
                prompt: Box::new(prompt),
            }),
        }
    }

    /// The server refused what it was given: forget it, and tell the source.
    fn refused(&self, bearer: bool) {
        let mut auth = lock(&self.auth);
        auth.cred = None;
        auth.scheme = None;
        drop(auth);
        self.credentials.rejected(&self.key, &self.prompt(bearer));
    }

    fn authorization(&self, method: &str, url: &Url) -> Option<HeaderValue> {
        if origin_of(url) != self.origin {
            return None;
        }
        let mut auth = lock(&self.auth);
        let AuthState { cred, scheme } = &mut *auth;
        let (cred, scheme) = (cred.as_ref()?, scheme.as_mut()?);
        let user = cred.user.as_deref().or(self.user.as_deref());
        let value = match scheme {
            Scheme::Basic => {
                let mut pair = format!("{}:", user?).into_bytes();
                pair.extend_from_slice(cred.secret.expose());
                format!("Basic {}", STANDARD.encode(pair))
            }
            Scheme::Bearer => format!("Bearer {}", cred.secret.expose_str()?),
            Scheme::Digest(state) => {
                let target = match url.query() {
                    Some(query) => format!("{}?{query}", url.path()),
                    None => url.path().to_owned(),
                };
                state.authorize(method, &target, user?, cred.secret.expose())
            }
        };
        let mut header = HeaderValue::from_str(&value).ok()?;
        header.set_sensitive(true);
        Some(header)
    }

    /// Picks the scheme to answer with from what the server offered.
    fn choose(&self, offered: &[Challenge]) -> Option<Challenge> {
        let digest = offered.iter().find(|c| matches!(c, Challenge::Digest(_)));
        let basic = offered.iter().find(|c| matches!(c, Challenge::Basic));
        let bearer = offered.iter().find(|c| matches!(c, Challenge::Bearer));
        match self.options.auth {
            AuthMode::Auto => digest.or(basic).or(bearer),
            AuthMode::Basic => basic,
            AuthMode::Digest => digest,
            AuthMode::Bearer => bearer,
        }
        .cloned()
    }

    /// Before the first request, a connection set to Basic or a token has what it needs: it sends
    /// the login at once instead of waiting to be asked.
    fn prepare(&self, location: &Location) -> Result<(), VfsError> {
        let (bearer, scheme) = match self.options.auth {
            AuthMode::Basic => (false, Scheme::Basic),
            AuthMode::Bearer => (true, Scheme::Bearer),
            _ => return Ok(()),
        };
        if lock(&self.auth).scheme.is_some() {
            return Ok(());
        }
        self.ensure_credential(bearer, location)?;
        lock(&self.auth).scheme = Some(scheme);
        Ok(())
    }

    /// Sends `request` and returns the response whatever its status, except that a challenge is
    /// answered, a redirect followed, a request to slow down is an error, and a failure to connect
    /// is the typed error that says why. A change is never sent twice by itself.
    pub(crate) async fn send(
        &self,
        request: &Request,
        location: &Location,
        cancel: Option<&CancelToken>,
    ) -> Result<Reply, VfsError> {
        let permit = match cancel {
            Some(cancel) => tokio::select! {
                permit = self.gate.clone().acquire_owned() => permit,
                () = cancelled(cancel) => return Err(VfsError::Cancelled),
            },
            None => self.gate.clone().acquire_owned().await,
        }
        .map_err(|_| VfsError::Disconnected {
            location: location.clone(),
        })?;
        self.prepare(location)?;
        let mut url = Url::parse(&request.url).map_err(|_| VfsError::InvalidLocation {
            input: request.url.clone(),
        })?;
        let (mut hops, mut challenges) = (0, 0);
        loop {
            let authorization = self.authorization(request.method, &url);
            let sent_login = authorization.is_some();
            let response = self
                .once(request, &url, authorization, location, cancel)
                .await?;
            let status = response.status();
            if status == StatusCode::UNAUTHORIZED && origin_of(&url) == self.origin {
                let values: Vec<&str> = response
                    .headers()
                    .get_all("www-authenticate")
                    .iter()
                    .filter_map(|v| v.to_str().ok())
                    .collect();
                let offered = parse_challenges(&values);
                challenges += 1;
                // A digest login whose nonce merely went stale is answered again, once.
                let stale = sent_login
                    && offered
                        .iter()
                        .any(|c| matches!(c, Challenge::Digest(d) if d.stale));
                if sent_login && !stale || challenges > 3 {
                    let bearer = matches!(lock(&self.auth).scheme, Some(Scheme::Bearer));
                    self.refused(bearer);
                    return Err(VfsError::AuthFailed {
                        location: location.clone(),
                    });
                }
                let Some(chosen) = self.choose(&offered) else {
                    return Err(VfsError::Io {
                        message: "the server asks for a kind of login this version of Waypoint \
                                  does not speak"
                            .to_owned(),
                        location: Some(location.clone()),
                    });
                };
                let bearer = matches!(chosen, Challenge::Bearer);
                self.ensure_credential(bearer, location)?;
                lock(&self.auth).scheme = Some(match chosen {
                    Challenge::Basic => Scheme::Basic,
                    Challenge::Bearer => Scheme::Bearer,
                    Challenge::Digest(challenge) => Scheme::Digest(DigestState::new(challenge)),
                });
                continue;
            }
            if matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308) {
                if let Some(next) = response
                    .headers()
                    .get(LOCATION)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| url.join(v).ok())
                {
                    hops += 1;
                    if hops > MAX_REDIRECTS {
                        return Err(VfsError::Io {
                            message: "the server redirected the request too many times".to_owned(),
                            location: Some(location.clone()),
                        });
                    }
                    if url.scheme() == "https" && next.scheme() != "https" {
                        return Err(VfsError::Io {
                            message: "the server redirected a secure connection to an insecure \
                                      address, which Waypoint will not follow"
                                .to_owned(),
                            location: Some(location.clone()),
                        });
                    }
                    url = next;
                    continue;
                }
            }
            if status == StatusCode::TOO_MANY_REQUESTS || status == StatusCode::SERVICE_UNAVAILABLE
            {
                let retry = header_text(response.headers(), RETRY_AFTER)
                    .and_then(|value| retry_after(&value));
                if status == StatusCode::TOO_MANY_REQUESTS || retry.is_some() {
                    return Err(crate::errors::from_status(
                        crate::errors::Op::Read,
                        status,
                        retry,
                        location,
                    ));
                }
            }
            return Ok(Reply {
                response,
                _permit: permit,
            });
        }
    }

    /// One attempt, to one URL.
    async fn once(
        &self,
        request: &Request,
        url: &Url,
        authorization: Option<HeaderValue>,
        location: &Location,
        cancel: Option<&CancelToken>,
    ) -> Result<reqwest::Response, VfsError> {
        let method =
            Method::from_bytes(request.method.as_bytes()).map_err(|_| VfsError::Unsupported {
                what: format!("the {} method", request.method),
            })?;
        let mut builder = self.client.request(method, url.clone());
        for (name, value) in &request.headers {
            if let (Ok(name), Ok(value)) = (
                HeaderName::from_bytes(name.as_bytes()),
                HeaderValue::from_str(value),
            ) {
                builder = builder.header(name, value);
            }
        }
        if let Some(authorization) = authorization {
            builder = builder.header(AUTHORIZATION, authorization);
        }
        builder = match &request.payload {
            Payload::None => builder,
            Payload::Bytes(bytes) => builder.body(bytes.clone()),
            Payload::Upload(upload) => {
                let body = upload.body().await.map_err(|error| VfsError::Io {
                    message: format!("the upload could not be read back: {error}"),
                    location: Some(location.clone()),
                })?;
                builder
                    .header(reqwest::header::CONTENT_LENGTH, upload.len)
                    .body(body)
            }
        };
        self.trust.clear_rejected();
        let sending = builder.send();
        let result = match cancel {
            Some(cancel) => tokio::select! {
                result = sending => result,
                () = cancelled(cancel) => return Err(VfsError::Cancelled),
            },
            None => sending.await,
        };
        result.map_err(|error| match self.trust.take_rejected() {
            Some(rejected) => VfsError::CertificateUntrusted {
                location: location.clone(),
                certificate: Box::new(describe(&rejected, self.trust.has_trusted())),
            },
            None => from_transport(&error, location),
        })
    }
}

fn origin_of(url: &Url) -> String {
    url.origin().ascii_serialization()
}

fn header_text(headers: &HeaderMap, name: HeaderName) -> Option<String> {
    headers.get(name)?.to_str().ok().map(str::to_owned)
}

/// Resolves when `cancel` is set.
pub(crate) async fn cancelled(cancel: &CancelToken) {
    while !cancel.is_cancelled() {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
