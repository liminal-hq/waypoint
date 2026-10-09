// Fixtures for testing the elevated provider, and what is built on it, without a privileged
// process: an in-memory duplex stream and a launcher that serves on a thread. For tests only.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

pub mod loopback;
pub mod pipe;
