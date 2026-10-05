// The connections of one provider: a client per region of each bucket's login, the credential it
// signs with, and the one place every request goes through.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::error::Error as StdError;
use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use aws_sdk_s3::config::http::HttpResponse;
use aws_sdk_s3::config::retry::RetryConfig;
use aws_sdk_s3::config::timeout::TimeoutConfig;
use aws_sdk_s3::config::{
    BehaviorVersion, Credentials, Region, RequestChecksumCalculation, ResponseChecksumValidation,
    SharedHttpClient,
};
use aws_sdk_s3::error::{ProvideErrorMetadata, SdkError};
use aws_sdk_s3::Client;
use waypoint_path::ConnectionKey;
use waypoint_protocol::{AuthPrompt, ConnectionState, Location, VfsError};
use waypoint_vfs::{CancelToken, Credential, Secret};

use crate::address::Address;
use crate::ambient;
use crate::errors::{from_sdk, S3Error};
use crate::options::{S3Config, S3Options};
use crate::presets::Preset;

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// A request that failed: before it left (no credential to sign with) or after.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Failed {
    Vfs(VfsError),
    S3(S3Error),
}

impl Failed {
    pub(crate) fn into_vfs(self, location: &Location) -> VfsError {
        match self {
            Failed::Vfs(error) => error,
            Failed::S3(error) => error.into_vfs(location),
        }
    }

    pub(crate) fn is(&self, error: &S3Error) -> bool {
        matches!(self, Failed::S3(own) if own == error)
    }
}

impl From<S3Error> for Failed {
    fn from(error: S3Error) -> Self {
        Failed::S3(error)
    }
}

/// What requests are signed with.
#[derive(Clone)]
pub(crate) enum Creds {
    Keys {
        key_id: String,
        secret: Secret,
        token: Option<Secret>,
        /// A region the source knew (the AWS profile's).
        region: Option<String>,
    },
    Anonymous,
}

struct State {
    creds: Option<Creds>,
    clients: HashMap<String, Client>,
    bucket_region: Option<String>,
    state: ConnectionState,
}

/// One bucket's login: what `ConnectionKey` names.
pub(crate) struct Conn {
    pub(crate) key: ConnectionKey,
    state: Mutex<State>,
}

impl Conn {
    fn new(key: ConnectionKey) -> Self {
        Self {
            key,
            state: Mutex::new(State {
                creds: None,
                clients: HashMap::new(),
                bucket_region: None,
                state: ConnectionState::Idle,
            }),
        }
    }

    pub(crate) fn state(&self) -> ConnectionState {
        lock(&self.state).state.clone()
    }

    fn set_state(&self, state: ConnectionState) {
        lock(&self.state).state = state;
    }

    /// Forgets the clients (and so their connections), keeping the credential.
    fn reset_clients(&self) {
        lock(&self.state).clients.clear();
    }

    pub(crate) fn set_credentials(&self, creds: Creds) {
        let mut state = lock(&self.state);
        state.creds = Some(creds);
        state.clients.clear();
    }

    pub(crate) fn disconnect(&self) {
        let mut state = lock(&self.state);
        state.clients.clear();
        state.state = ConnectionState::Idle;
    }
}

pub(crate) struct Inner {
    pub(crate) config: S3Config,
    conns: Mutex<HashMap<ConnectionKey, Arc<Conn>>>,
    options: Mutex<HashMap<ConnectionKey, S3Options>>,
    runtime: OnceLock<tokio::runtime::Runtime>,
    http: OnceLock<SharedHttpClient>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        // Dropping a runtime waits for its tasks unless told not to, which may be on a thread
        // that must not block.
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

/// The host of an origin (`https://minio.lan:9000` is `minio.lan`).
fn host_of(origin: &str) -> &str {
    let rest = origin.split_once("://").map_or(origin, |(_, rest)| rest);
    let rest = rest.split('/').next().unwrap_or(rest);
    if let Some(inner) = rest.strip_prefix('[') {
        return inner.split(']').next().unwrap_or(inner);
    }
    rest.split(':').next().unwrap_or(rest)
}

impl Inner {
    pub(crate) fn new(config: S3Config) -> Self {
        Self {
            config,
            conns: Mutex::new(HashMap::new()),
            options: Mutex::new(HashMap::new()),
            runtime: OnceLock::new(),
            http: OnceLock::new(),
        }
    }

    pub(crate) fn runtime(&self) -> &tokio::runtime::Runtime {
        self.runtime.get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .thread_name("waypoint-s3")
                .enable_all()
                .build()
                .expect("the S3 runtime starts")
        })
    }

    pub(crate) fn conn(&self, key: &ConnectionKey) -> Arc<Conn> {
        lock(&self.conns)
            .entry(key.clone())
            .or_insert_with(|| Arc::new(Conn::new(key.clone())))
            .clone()
    }

    pub(crate) fn existing_conn(&self, key: &ConnectionKey) -> Option<Arc<Conn>> {
        lock(&self.conns).get(key).cloned()
    }

    pub(crate) fn set_options(&self, key: &ConnectionKey, options: S3Options) {
        lock(&self.options).insert(key.clone(), options);
        if let Some(conn) = self.existing_conn(key) {
            conn.reset_clients();
        }
    }

    pub(crate) fn options(&self, key: &ConnectionKey) -> S3Options {
        lock(&self.options)
            .get(key)
            .cloned()
            .unwrap_or_else(|| self.config.options.clone())
    }

    /// The preset of the service an address is on.
    pub(crate) fn preset(&self, address: &Address) -> &'static Preset {
        Preset::for_host(address.endpoint.as_deref().map(host_of))
    }

    pub(crate) fn conditional_writes(&self, address: &Address) -> bool {
        self.options(&address.connection)
            .conditional_writes
            .unwrap_or(self.preset(address).conditional_write)
    }

    fn follows_redirects(&self, address: &Address) -> bool {
        self.options(&address.connection)
            .follow_region_redirects
            .unwrap_or(self.preset(address).id == "aws")
    }

    fn http_client(&self) -> SharedHttpClient {
        self.http
            .get_or_init(|| {
                // `ring` rather than the SDK's default `aws-lc`, as the SSH provider does: no C
                // build, and the same TLS stack on Windows. The system's roots are trusted.
                aws_smithy_http_client::Builder::new()
                    .tls_provider(aws_smithy_http_client::tls::Provider::Rustls(
                        aws_smithy_http_client::tls::rustls_provider::CryptoMode::Ring,
                    ))
                    .build_https()
            })
            .clone()
    }

    fn prompt() -> AuthPrompt {
        AuthPrompt::AccessKey { key_id: None }
    }

    /// The credential to sign `address`'s requests with: this session's answer, the app's source,
    /// the ambient AWS credentials (AWS endpoints only), or none for a public bucket.
    fn credentials(&self, conn: &Conn, address: &Address) -> Result<Creds, VfsError> {
        if let Some(creds) = lock(&conn.state).creds.clone() {
            return Ok(creds);
        }
        let options = self.options(&conn.key);
        let creds = if options.anonymous {
            Some(Creds::Anonymous)
        } else if let Some(Credential::AccessKey { key_id, secret }) = self
            .config
            .credentials
            .credential(&conn.key, &Self::prompt())
        {
            Some(Creds::Keys {
                key_id,
                secret,
                token: options.session_token.clone(),
                region: None,
            })
        } else if self.config.ambient_credentials && self.preset(address).id == "aws" {
            ambient::read().map(|found| Creds::Keys {
                key_id: found.key_id,
                secret: found.secret,
                token: found.session_token,
                region: found.region,
            })
        } else {
            None
        };
        match creds {
            Some(creds) => {
                lock(&conn.state).creds = Some(creds.clone());
                Ok(creds)
            }
            None => Err(VfsError::AuthRequired {
                location: address.location.clone(),
                prompt: Box::new(Self::prompt()),
            }),
        }
    }

    fn region(&self, conn: &Conn, address: &Address, creds: &Creds) -> String {
        if let Some(region) = lock(&conn.state).bucket_region.clone() {
            return region;
        }
        let options = self.options(&conn.key);
        if let Some(region) = options.region {
            return region;
        }
        let preset = self.preset(address);
        if let Some(origin) = &address.endpoint {
            if let Some(region) = preset.region_in_host(host_of(origin)) {
                return region;
            }
        } else if let Creds::Keys {
            region: Some(region),
            ..
        } = creds
        {
            return region.clone();
        }
        preset.default_region.to_owned()
    }

    fn build(
        &self,
        address: &Address,
        region: &str,
        creds: &Creds,
        options: &S3Options,
    ) -> Result<Client, VfsError> {
        let path_style = options
            .path_style
            .unwrap_or(self.preset(address).path_style);
        let mut builder = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new(region.to_owned()))
            .http_client(self.http_client())
            .force_path_style(path_style)
            // Only checksum what the request requires: the newer default adds a trailer that
            // older S3-compatible services (and some current ones) reject.
            .request_checksum_calculation(RequestChecksumCalculation::WhenRequired)
            .response_checksum_validation(ResponseChecksumValidation::WhenRequired)
            .retry_config(
                RetryConfig::standard().with_max_attempts(options.max_attempts.unwrap_or(4)),
            )
            .timeout_config(
                TimeoutConfig::builder()
                    .connect_timeout(Duration::from_secs(10))
                    .read_timeout(Duration::from_secs(60))
                    .build(),
            );
        if let Some(origin) = &address.endpoint {
            builder = builder.endpoint_url(origin.clone());
        }
        match creds {
            Creds::Keys {
                key_id,
                secret,
                token,
                ..
            } => {
                let secret = secret.expose_str().ok_or_else(|| VfsError::AuthFailed {
                    location: address.location.clone(),
                })?;
                let token = token
                    .as_ref()
                    .and_then(|token| token.expose_str().map(str::to_owned));
                builder = builder.credentials_provider(Credentials::new(
                    key_id.clone(),
                    secret.to_owned(),
                    token,
                    None,
                    "waypoint",
                ));
            }
            Creds::Anonymous => builder = builder.allow_no_auth(),
        }
        Ok(Client::from_conf(builder.build()))
    }

    /// The client for `address`'s bucket (in the region it is known to be in), made on first use.
    pub(crate) fn client(&self, address: &Address) -> Result<(Client, Arc<Conn>), VfsError> {
        let conn = self.conn(&address.connection);
        let creds = match self.credentials(&conn, address) {
            Ok(creds) => creds,
            Err(error) => {
                conn.set_state(ConnectionState::Failed {
                    error: error.clone(),
                });
                return Err(error);
            }
        };
        let region = self.region(&conn, address, &creds);
        if let Some(client) = lock(&conn.state).clients.get(&region) {
            return Ok((client.clone(), conn.clone()));
        }
        let client = self.build(address, &region, &creds, &self.options(&conn.key))?;
        lock(&conn.state).clients.insert(region, client.clone());
        Ok((client, conn))
    }

    /// The server refused the credential: forget it everywhere so nothing offers it again unasked.
    fn rejected(&self, conn: &Conn, address: &Address) {
        self.config.credentials.rejected(&conn.key, &Self::prompt());
        let mut state = lock(&conn.state);
        state.creds = None;
        state.clients.clear();
        state.state = ConnectionState::Failed {
            error: VfsError::AuthFailed {
                location: address.location.clone(),
            },
        };
    }

    /// Sends a request through `address`'s client, and follows one region redirect (AWS only).
    /// `send` may run twice, so it makes the request afresh each time.
    pub(crate) async fn run<T, E, F, Fut>(
        &self,
        address: &Address,
        mut send: F,
    ) -> Result<T, Failed>
    where
        F: FnMut(Client) -> Fut,
        Fut: Future<Output = Result<T, SdkError<E, HttpResponse>>>,
        E: ProvideErrorMetadata + StdError + 'static,
    {
        let mut redirected = false;
        loop {
            let (client, conn) = self.client(address).map_err(Failed::Vfs)?;
            let error = match send(client.clone()).await {
                Ok(value) => {
                    conn.set_state(ConnectionState::Connected);
                    return Ok(value);
                }
                Err(error) => error,
            };
            let failure = from_sdk(&error);
            match &failure {
                S3Error::Redirect { region } if !redirected && self.follows_redirects(address) => {
                    redirected = true;
                    let region = match region {
                        Some(region) => Some(region.clone()),
                        None => self.discover_region(&client, address).await,
                    };
                    if let Some(region) = region {
                        lock(&conn.state).bucket_region = Some(region);
                        continue;
                    }
                }
                failure if failure.is_credential_failure() => self.rejected(&conn, address),
                failure if failure.ends_session() => {
                    conn.reset_clients();
                    conn.set_state(ConnectionState::Failed {
                        error: failure.clone().into_vfs(&address.location),
                    });
                }
                _ => {}
            }
            return Err(Failed::S3(failure));
        }
    }

    /// Asks the service which region the bucket is in (`HeadBucket` answers with it, success or
    /// not).
    async fn discover_region(&self, client: &Client, address: &Address) -> Option<String> {
        match client.head_bucket().bucket(&address.bucket).send().await {
            Ok(output) => output.bucket_region().map(str::to_owned),
            Err(error) => match from_sdk(&error) {
                S3Error::Redirect { region } => region,
                _ => None,
            },
        }
    }
}

/// Waits for `future`, giving up with `Cancelled` soon after `cancel` is set.
pub(crate) async fn cancellable<T>(
    cancel: &CancelToken,
    future: impl Future<Output = Result<T, Failed>>,
) -> Result<T, Failed> {
    tokio::pin!(future);
    loop {
        tokio::select! {
            result = &mut future => return result,
            _ = tokio::time::sleep(Duration::from_millis(50)) => {
                if cancel.is_cancelled() {
                    return Err(Failed::Vfs(VfsError::Cancelled));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_host_of_an_origin() {
        assert_eq!(host_of("https://minio.lan:9000"), "minio.lan");
        assert_eq!(host_of("http://[::1]:9000"), "::1");
        assert_eq!(host_of("https://s3.wasabisys.com"), "s3.wasabisys.com");
    }
}
