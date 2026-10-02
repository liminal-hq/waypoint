// Fixtures for testing the engine and what is built on it: a provider that panics outside its
// root, one that fails on a script, a Trash kept in a provider, tree helpers and a harness that
// ties them together.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

pub mod faulty;
pub mod harness;
pub mod journal_harness;
pub mod journal_storage;
pub mod sandbox;
pub mod transfer;
pub mod trash;
pub mod tree;
