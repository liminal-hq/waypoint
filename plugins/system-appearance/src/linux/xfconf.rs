// Reads and monitors xfwm4 titlebar preferences through xfconf-query
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tokio::sync::mpsc::UnboundedSender;

use super::cli;
use crate::{
    models::{
        DesktopEnvironment, LayoutSource, TitlebarAction, TitlebarActions, TitlebarPreferences,
    },
    parse,
};

fn get(property: &str) -> Result<String, String> {
    cli::run("xfconf-query", &["-c", "xfwm4", "-p", property])
}

/// Reads the xfwm4 button layout and double-click action; fails when the layout is unreadable.
pub fn read() -> Result<TitlebarPreferences, String> {
    let layout = get("/general/button_layout")?;
    let double_click = get("/general/double_click_action")
        .map_or(TitlebarAction::ToggleMaximise, |value| {
            parse::xfce_double_click(&value)
        });
    Ok(TitlebarPreferences {
        button_layout: parse::xfce_button_layout(&layout),
        actions: TitlebarActions {
            double_click,
            ..TitlebarActions::DEFAULT
        },
        desktop_environment: DesktopEnvironment::Xfce,
        source: LayoutSource::Xfconf,
    })
}

pub fn watch(changed: UnboundedSender<()>) -> Vec<Box<dyn Send>> {
    cli::monitor_guard("xfconf-query", &["-c", "xfwm4", "-m"], changed)
}
