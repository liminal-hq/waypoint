// Reports every feature unavailable on targets without tear-off support, such as macOS, Android and iOS
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::status::Platform;

pub fn platform() -> Platform {
    Platform::Unsupported
}

pub fn buttons_held() -> bool {
    false
}
