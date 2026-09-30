// Reads and watches KWin's kwinrc for titlebar button and action preferences
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    ffi::OsStr,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use notify::{RecursiveMode, Watcher as _};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    models::{DesktopEnvironment, LayoutSource, TitlebarPreferences},
    parse,
};

const FILE_NAME: &str = "kwinrc";

/// Resolves the user config directory from `XDG_CONFIG_HOME`, falling back to `~/.config`.
fn config_home(xdg_config_home: Option<&str>, home: Option<&str>) -> Option<PathBuf> {
    let absolute = |value: &&str| Path::new(value).is_absolute();
    xdg_config_home
        .filter(absolute)
        .map(PathBuf::from)
        .or_else(|| {
            home.filter(absolute)
                .map(|home| Path::new(home).join(".config"))
        })
}

fn kwinrc_path() -> Option<PathBuf> {
    let dir = config_home(
        std::env::var("XDG_CONFIG_HOME").ok().as_deref(),
        std::env::var("HOME").ok().as_deref(),
    )?;
    Some(dir.join(FILE_NAME))
}

/// Reads the KWin preferences. A missing file means KWin's defaults are in effect.
pub fn read() -> Result<TitlebarPreferences, String> {
    let path = kwinrc_path().ok_or("cannot locate the user config directory")?;
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    Ok(TitlebarPreferences {
        button_layout: parse::kde_layout_from_kwinrc(&text),
        actions: parse::kde_actions(&text),
        desktop_environment: DesktopEnvironment::Kde,
        source: LayoutSource::KwinConfig,
    })
}

/// Watches the config directory rather than the file, because KDE and editors replace it atomically.
pub fn watch(changed: UnboundedSender<()>) -> Vec<Box<dyn Send>> {
    let Some(path) = kwinrc_path() else {
        return Vec::new();
    };
    let Some(dir) = path.parent().map(Path::to_path_buf) else {
        return Vec::new();
    };
    let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        if let Ok(event) = event {
            if event
                .paths
                .iter()
                .any(|p| p.file_name() == Some(OsStr::new(FILE_NAME)))
            {
                let _ = changed.send(());
            }
        }
    })
    .and_then(|mut watcher| {
        watcher.watch(&dir, RecursiveMode::NonRecursive)?;
        Ok(watcher)
    });
    match watcher {
        Ok(watcher) => vec![Box::new(watcher)],
        Err(error) => {
            log::warn!("system-appearance: cannot watch {}: {error}", dir.display());
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_home_table() {
        let cases = [
            (Some("/x/cfg"), Some("/home/u"), Some("/x/cfg")),
            (None, Some("/home/u"), Some("/home/u/.config")),
            (Some(""), Some("/home/u"), Some("/home/u/.config")),
            (Some("relative"), Some("/home/u"), Some("/home/u/.config")),
            (None, None, None),
            (None, Some("relative"), None),
        ];
        for (xdg, home, expected) in cases {
            assert_eq!(
                config_home(xdg, home),
                expected.map(PathBuf::from),
                "{xdg:?} {home:?}"
            );
        }
    }
}
