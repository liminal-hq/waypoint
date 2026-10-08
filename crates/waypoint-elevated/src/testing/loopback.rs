// A launcher that serves on a thread of the same process, so the client and the helper's serve loop
// run against each other in a test.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use waypoint_protocol::VfsError;
use waypoint_vfs::{CancelToken, Provider};

use super::pipe::{duplex, Cutter};
use crate::client::{Launcher, Transport};
use crate::server::{serve, ServeConfig, ServeEnd};
use crate::sync::{locked, wait_timeout};

#[derive(Default)]
struct Record {
    launches: AtomicUsize,
    ends: Mutex<Vec<ServeEnd>>,
    ended: Condvar,
    cutters: Mutex<Vec<Cutter>>,
}

/// Starts `serve` against a provider on a new thread for each launch. Clones share what they count.
#[derive(Clone)]
pub struct LoopbackLauncher {
    provider: Arc<dyn Provider>,
    config: ServeConfig,
    record: Arc<Record>,
    refuse: Arc<Mutex<Option<VfsError>>>,
}

impl LoopbackLauncher {
    pub fn new(provider: Arc<dyn Provider>, config: ServeConfig) -> Self {
        Self {
            provider,
            config,
            record: Arc::default(),
            refuse: Arc::default(),
        }
    }

    /// How many times a helper has been started.
    pub fn launches(&self) -> usize {
        self.record.launches.load(Ordering::SeqCst)
    }

    /// Makes the next launches fail with `error`, as a refused prompt does; `None` lets them work.
    pub fn refuse_with(&self, error: Option<VfsError>) {
        *locked(&self.refuse) = error;
    }

    /// Cuts every stream made so far, as a helper that is killed.
    pub fn sever(&self) {
        for cutter in locked(&self.record.cutters).iter() {
            cutter.cut();
        }
    }

    /// How each serve loop that has returned ended.
    pub fn ends(&self) -> Vec<ServeEnd> {
        locked(&self.record.ends).clone()
    }

    /// Waits until `count` serve loops have returned, or `patience` runs out.
    pub fn wait_for_ends(&self, count: usize, patience: Duration) -> Vec<ServeEnd> {
        let started = Instant::now();
        let mut ends = locked(&self.record.ends);
        while ends.len() < count {
            let left = patience.saturating_sub(started.elapsed());
            if left.is_zero() {
                break;
            }
            ends = wait_timeout(&self.record.ended, ends, left);
        }
        ends.clone()
    }
}

impl Launcher for LoopbackLauncher {
    fn launch(&self, _cancel: &CancelToken) -> Result<Transport, VfsError> {
        self.record.launches.fetch_add(1, Ordering::SeqCst);
        if let Some(error) = locked(&self.refuse).clone() {
            return Err(error);
        }
        let streams = duplex();
        locked(&self.record.cutters).push(streams.cutter.clone());
        let (provider, config, record) = (
            self.provider.clone(),
            self.config.clone(),
            self.record.clone(),
        );
        let (reader, writer) = (streams.server_reader, streams.server_writer);
        std::thread::Builder::new()
            .name("elevated-loopback".to_owned())
            .spawn(move || {
                let end = serve(reader, writer, provider, config);
                locked(&record.ends).push(end);
                record.ended.notify_all();
            })
            .map_err(|_| VfsError::Io {
                message: "a thread could not be started".to_owned(),
                location: None,
            })?;
        Ok(Transport {
            reader: Box::new(streams.client_reader),
            writer: Box::new(streams.client_writer),
        })
    }
}
