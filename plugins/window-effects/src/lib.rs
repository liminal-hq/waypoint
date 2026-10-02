// Registers the window-effects plugin: report which window effects work and apply them per window, on Windows and Linux
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod error;
pub mod models;
pub mod request;
pub mod status;

pub use error::{Result, WindowEffectsError};
pub use models::*;
pub use status::{Desktop, Environment, SessionType};
