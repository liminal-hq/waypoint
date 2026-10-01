// Reads the 12/24-hour setting from the user's Windows Region settings and polls for changes
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::time::Duration;

use tokio::sync::mpsc::UnboundedSender;
use windows::{
    core::PCWSTR,
    Win32::Globalization::{
        GetLocaleInfoEx, LOCALE_ITIME, LOCALE_NAME_SYSTEM_DEFAULT, LOCALE_RETURN_NUMBER,
        LOCALE_SSHORTTIME, LOCALE_STIMEFORMAT,
    },
};

use crate::{
    models::{TimeFormat, TimeFormatSource},
    parse,
    service::{Poller, Readiness, Reading},
};

/// `LOCALE_NAME_USER_DEFAULT` is a null locale name, which the metadata does not define as a constant.
const USER_DEFAULT: PCWSTR = PCWSTR::null();

/// How often the Region settings are read again; the call is a cheap in-process lookup.
const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Windows has no change notification this plugin listens to, so it polls.
pub struct Watcher(#[allow(dead_code)] Poller);

impl Watcher {
    pub fn take_readiness(&mut self) -> Readiness {
        Readiness::Listening
    }
}

/// Reads a string-valued locale field of `locale`; `None` when the call fails or returns nothing.
fn locale_string(locale: PCWSTR, field: u32) -> Option<String> {
    let mut buffer = [0u16; 80];
    // SAFETY: the buffer outlives the call, and the length the API sees is the buffer's own.
    let length = unsafe { GetLocaleInfoEx(locale, field, Some(&mut buffer)) };
    // The count includes the terminating nul.
    let length = usize::try_from(length).ok().filter(|&n| n > 1)?;
    Some(String::from_utf16_lossy(&buffer[..length - 1]))
}

/// Reads `LOCALE_ITIME` of `locale`: 0 for a 12-hour clock and 1 for a 24-hour one.
fn time_mode(locale: PCWSTR) -> Option<u32> {
    let mut number = 0u32;
    // SAFETY: with `LOCALE_RETURN_NUMBER` the API writes a `DWORD` into the buffer, which is the
    // two `u16`s of `number`; the slice covers exactly that memory for the duration of the call.
    let length = unsafe {
        let buffer =
            std::slice::from_raw_parts_mut(std::ptr::addr_of_mut!(number).cast::<u16>(), 2);
        GetLocaleInfoEx(locale, LOCALE_ITIME | LOCALE_RETURN_NUMBER, Some(buffer))
    };
    (length > 0).then_some(number)
}

/// What a locale says about the clock: `None` when none of its fields could be read or told.
fn read_locale(locale: PCWSTR) -> Option<bool> {
    parse::windows_is_24_hour(
        locale_string(locale, LOCALE_SSHORTTIME).as_deref(),
        locale_string(locale, LOCALE_STIMEFORMAT).as_deref(),
        time_mode(locale),
    )
}

pub async fn read() -> Reading {
    // The user default locale carries the user's Region customisations. The system default
    // locale is only a fallback, for when the user's settings cannot be read at all.
    if let Some(is_24_hour) = read_locale(USER_DEFAULT) {
        return Reading {
            format: TimeFormat {
                is_24_hour,
                source: TimeFormatSource::WindowsUserLocale,
            },
            note: None,
        };
    }
    match read_locale(LOCALE_NAME_SYSTEM_DEFAULT) {
        Some(is_24_hour) => Reading {
            format: TimeFormat {
                is_24_hour,
                source: TimeFormatSource::WindowsFallback,
            },
            note: Some(
                "the user's Region settings could not be read; using the system locale".to_string(),
            ),
        },
        None => Reading {
            format: TimeFormat {
                is_24_hour: false,
                source: TimeFormatSource::Default,
            },
            note: Some("Windows did not report a time format".to_string()),
        },
    }
}

pub fn watch(changed: UnboundedSender<()>) -> Watcher {
    Watcher(Poller::start(changed, POLL_INTERVAL))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live check on a Windows machine: prints what Windows reports every two seconds for `WATCH_SECONDS`
    /// (default 10), so the Region settings can be changed while it runs. Run with
    /// `cargo test -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_region_read() {
        let seconds: u64 = std::env::var("WATCH_SECONDS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(10);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        for _ in 0..=seconds / 2 {
            let reading = runtime.block_on(read());
            println!(
                "short={:?} long={:?} itime={:?} -> {:?}",
                locale_string(USER_DEFAULT, LOCALE_SSHORTTIME),
                locale_string(USER_DEFAULT, LOCALE_STIMEFORMAT),
                time_mode(USER_DEFAULT),
                reading
            );
            std::thread::sleep(Duration::from_secs(2));
        }
    }
}
