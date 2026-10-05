// How long a job waits for a server that stopped answering before it tries again by itself, and how
// many times it tries before it asks (D165).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use waypoint_protocol::VfsError;

use crate::model::OpsError;

/// How many times in a row a job tries again by itself before it asks: with the waits below, about
/// three minutes.
pub const RECONNECT_ATTEMPTS: u32 = 10;

/// The waits between tries, in seconds, the last repeated.
const BACKOFF_SECONDS: [u64; 6] = [1, 2, 4, 8, 15, 30];

/// The longest a server's own "slow down" is waited for in one go.
const MAX_RATE_WAIT_MS: u64 = 60_000;

/// Whether `error` is a connection that went away or a server that stopped answering, which may
/// come back by itself; a login, a host key or a certificate needs the person and is not.
pub fn is_transient(error: &OpsError) -> bool {
    matches!(
        error,
        OpsError::Connection {
            error: VfsError::Disconnected { .. }
                | VfsError::Unreachable { .. }
                | VfsError::Timeout { .. }
                | VfsError::RateLimited { .. }
        }
    )
}

/// How long to wait before the `attempt`th try again (from 0), or `None` when `error` does not
/// come back by itself or the tries are used up. A server that said how long to wait is waited for
/// that long (within a minute).
pub fn reconnect_delay_ms(error: &OpsError, attempt: u32) -> Option<u64> {
    if !is_transient(error) || attempt >= RECONNECT_ATTEMPTS {
        return None;
    }
    if let OpsError::Connection {
        error:
            VfsError::RateLimited {
                retry_after_ms: Some(ms),
                ..
            },
    } = error
    {
        return Some((*ms).clamp(1_000, MAX_RATE_WAIT_MS));
    }
    let step = (attempt as usize).min(BACKOFF_SECONDS.len() - 1);
    Some(BACKOFF_SECONDS[step] * 1_000)
}

#[cfg(test)]
mod tests {
    use super::*;
    use waypoint_protocol::{Location, UnreachableReason};

    fn at() -> Location {
        Location::new("sftp://h/x", "sftp://h/x")
    }

    #[test]
    fn a_lost_connection_backs_off_and_then_asks() {
        let lost = OpsError::Connection {
            error: VfsError::Disconnected { location: at() },
        };
        let waits: Vec<_> = (0..RECONNECT_ATTEMPTS + 1)
            .map(|n| reconnect_delay_ms(&lost, n))
            .collect();
        assert_eq!(
            waits[..7],
            [1_000, 2_000, 4_000, 8_000, 15_000, 30_000, 30_000].map(Some)
        );
        assert_eq!(waits[RECONNECT_ATTEMPTS as usize], None);
        let total: u64 = waits.iter().flatten().sum();
        assert!(total > 120_000 && total < 300_000, "{total}");
    }

    #[test]
    fn only_what_can_come_back_by_itself_is_waited_for() {
        let unreachable = OpsError::Connection {
            error: VfsError::Unreachable {
                location: at(),
                reason: UnreachableReason::Offline,
            },
        };
        assert!(is_transient(&unreachable));
        let login = OpsError::Connection {
            error: VfsError::AuthFailed { location: at() },
        };
        assert_eq!(reconnect_delay_ms(&login, 0), None);
        assert_eq!(
            reconnect_delay_ms(
                &OpsError::Io {
                    message: "x".into()
                },
                0
            ),
            None
        );
        let slow = OpsError::Connection {
            error: VfsError::RateLimited {
                location: at(),
                retry_after_ms: Some(120_000),
            },
        };
        assert_eq!(reconnect_delay_ms(&slow, 0), Some(60_000));
    }
}
