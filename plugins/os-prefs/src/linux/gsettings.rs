// Reads and monitors the clock setting through the `gsettings` tool
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tokio::sync::mpsc::UnboundedSender;

use super::cli;
use crate::parse;

const GNOME_SCHEMA: &str = "org.gnome.desktop.interface";
const GNOME_KEY: &str = "clock-format";
const CINNAMON_SCHEMA: &str = "org.cinnamon.desktop.interface";
const CINNAMON_KEY: &str = "clock-use-24h";

/// Reads GNOME's `clock-format`.
pub fn read_gnome() -> Result<bool, String> {
    let raw = cli::run("gsettings", &["get", GNOME_SCHEMA, GNOME_KEY])?;
    parse::gnome_clock_format(&raw).ok_or_else(|| format!("unexpected {GNOME_KEY} value {raw:?}"))
}

/// Reads Cinnamon's `clock-use-24h`.
pub fn read_cinnamon() -> Result<bool, String> {
    let raw = cli::run("gsettings", &["get", CINNAMON_SCHEMA, CINNAMON_KEY])?;
    parse::gvariant_bool(&raw).ok_or_else(|| format!("unexpected {CINNAMON_KEY} value {raw:?}"))
}

pub fn watch_cinnamon(changed: UnboundedSender<()>) -> Vec<Box<dyn Send>> {
    cli::monitor_guard(
        "gsettings",
        &["monitor", CINNAMON_SCHEMA, CINNAMON_KEY],
        changed,
    )
}
