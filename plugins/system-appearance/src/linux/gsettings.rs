// Reads and monitors titlebar preferences from GSettings schemas via the gsettings tool
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tokio::sync::mpsc::UnboundedSender;

use super::cli;
use crate::{
    models::{DesktopEnvironment, LayoutSource, TitlebarPreferences},
    parse,
};

pub const CINNAMON_SCHEMA: &str = "org.cinnamon.desktop.wm.preferences";
pub const MATE_SCHEMA: &str = "org.mate.Marco.general";

fn get(schema: &str, key: &str) -> Result<String, String> {
    let raw = cli::run("gsettings", &["get", schema, key])?;
    Ok(parse::unquote_gvariant_string(&raw).to_string())
}

/// Reads the titlebar preferences from `schema`; fails when the schema or layout key is missing.
pub fn read(
    schema: &str,
    desktop_environment: DesktopEnvironment,
) -> Result<TitlebarPreferences, String> {
    let layout = get(schema, "button-layout")?;
    let action = |key| get(schema, key).ok();
    let actions = parse::gnome_actions(
        action("action-double-click-titlebar").as_deref(),
        action("action-middle-click-titlebar").as_deref(),
        action("action-right-click-titlebar").as_deref(),
    );
    Ok(TitlebarPreferences {
        button_layout: parse::gnome_button_layout(&layout),
        actions,
        desktop_environment,
        source: LayoutSource::Gsettings,
    })
}

pub fn watch(schema: &str, changed: UnboundedSender<()>) -> Vec<Box<dyn Send>> {
    cli::monitor_guard("gsettings", &["monitor", schema], changed)
}
