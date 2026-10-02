// Turns what the Windows shell reports about the handlers of an extension into the plugin's handler list
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

use crate::models::{App, Handlers};

/// One association handler as `IAssocHandler` describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawHandler {
    /// `GetName`: the executable's path, or a package or ProgID name. The handler's id.
    pub name: String,
    /// `GetUIName`: the name to show.
    pub ui_name: String,
    /// `IsRecommended`: the application registered itself for the extension.
    pub recommended: bool,
    /// `GetIconLocation`, as `path,index`.
    pub icon: Option<String>,
}

/// The default application as `AssocQueryStringW` reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawDefault {
    pub executable: String,
    pub friendly_name: Option<String>,
}

fn app_of(handler: &RawHandler) -> App {
    let looks_like_a_command = handler.name.contains(['\\', '/']) || handler.name.ends_with(".exe");
    App {
        id: handler.name.clone(),
        name: if handler.ui_name.is_empty() {
            handler.name.clone()
        } else {
            handler.ui_name.clone()
        },
        icon: handler.icon.clone(),
        exec_hint: looks_like_a_command.then(|| handler.name.clone()),
    }
}

fn is_default(handler: &RawHandler, default: &RawDefault) -> bool {
    handler.name.eq_ignore_ascii_case(&default.executable)
        || default
            .friendly_name
            .as_deref()
            .is_some_and(|name| !name.is_empty() && handler.ui_name.eq_ignore_ascii_case(name))
}

/// Builds the lists: the default first (found among the handlers, or described by the default's own strings when the shell does not list it), then the recommended handlers, then the others by name. A handler is listed once, by its id.
pub fn build(mime: &str, handlers: &[RawHandler], default: Option<&RawDefault>) -> Handlers {
    let mut seen: Vec<String> = Vec::new();
    let mut unique: Vec<&RawHandler> = Vec::new();
    for handler in handlers {
        let key = handler.name.to_lowercase();
        if !handler.name.is_empty() && !seen.contains(&key) {
            seen.push(key);
            unique.push(handler);
        }
    }
    let default_app = default.map(|default| {
        match unique
            .iter()
            .position(|handler| is_default(handler, default))
        {
            Some(index) => app_of(unique.remove(index)),
            None => App {
                id: default.executable.clone(),
                name: default
                    .friendly_name
                    .clone()
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| default.executable.clone()),
                icon: None,
                exec_hint: Some(default.executable.clone()),
            },
        }
    });
    let (recommended, mut others): (Vec<&RawHandler>, Vec<&RawHandler>) =
        unique.into_iter().partition(|handler| handler.recommended);
    others.sort_by_key(|handler| (app_of(handler).name.to_lowercase(), handler.name.clone()));
    Handlers {
        mime: mime.to_string(),
        mixed: false,
        default: default_app,
        recommended: recommended.into_iter().map(app_of).collect(),
        others: others.into_iter().map(app_of).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(name: &str, ui: &str, recommended: bool) -> RawHandler {
        RawHandler {
            name: name.into(),
            ui_name: ui.into(),
            recommended,
            icon: Some(format!("{name},0")),
        }
    }

    #[test]
    fn the_default_leads_and_is_not_repeated() {
        let handlers = [
            raw(r"C:\Apps\paint.exe", "Paint", true),
            raw(r"C:\Apps\photos.exe", "Photos", true),
            raw(r"C:\Apps\zip.exe", "Zip", false),
            raw(r"C:\Apps\acme.exe", "Acme", false),
        ];
        let default = RawDefault {
            executable: r"c:\apps\photos.exe".into(),
            friendly_name: Some("Photos".into()),
        };
        let lists = build(".png", &handlers, Some(&default));
        assert_eq!(lists.default.unwrap().name, "Photos");
        let names = |apps: &[App]| apps.iter().map(|a| a.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&lists.recommended), ["Paint"]);
        assert_eq!(names(&lists.others), ["Acme", "Zip"]);
    }

    #[test]
    fn a_default_the_shell_does_not_list_is_described_by_its_own_strings() {
        let lists = build(
            ".png",
            &[raw(r"C:\Apps\paint.exe", "Paint", true)],
            Some(&RawDefault {
                executable: r"C:\Other\viewer.exe".into(),
                friendly_name: None,
            }),
        );
        let default = lists.default.unwrap();
        assert_eq!(default.id, r"C:\Other\viewer.exe");
        assert_eq!(default.name, r"C:\Other\viewer.exe");
        assert_eq!(lists.recommended.len(), 1);
    }

    #[test]
    fn a_handler_is_listed_once_and_one_without_a_name_not_at_all() {
        let lists = build(
            ".txt",
            &[
                raw(r"C:\a.exe", "A", false),
                raw(r"C:\A.EXE", "A again", false),
                raw("", "Nameless", false),
            ],
            None,
        );
        assert_eq!(lists.others.len(), 1);
        assert_eq!(lists.default, None);
    }

    #[test]
    fn a_package_name_is_an_id_but_not_a_command_line() {
        let lists = build(
            ".txt",
            &[raw("Microsoft.Notepad_8wekyb3d8bbwe", "Notepad", true)],
            None,
        );
        assert_eq!(lists.recommended[0].exec_hint, None);
        let lists = build(".txt", &[raw(r"C:\x\y.exe", "Y", true)], None);
        assert_eq!(
            lists.recommended[0].exec_hint.as_deref(),
            Some(r"C:\x\y.exe")
        );
    }
}
