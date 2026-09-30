// Waypoint's virtual file system. This slice holds only the wire types; the provider trait, the
// local provider and listing handles arrive in later slices of milestone 2.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod model;

pub use model::*;
