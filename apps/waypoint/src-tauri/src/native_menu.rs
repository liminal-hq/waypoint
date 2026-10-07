// Shows a context menu as the system's own menu and returns the item the person chose (experimental, D196)
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The page describes a menu (`NativeMenuItem`) and its position; this module checks the description,
// builds a `tauri::menu::Menu` from it, pops it up on the calling window and answers with the
// page's own id for the chosen item, or `None` when the menu was dismissed. The page owns what the
// ids mean (A146), so nothing here knows what a menu is for.
//
// The popup blocks the thread that shows it until the menu closes (a Win32 modal loop, or GTK's
// nested main iteration), and Tauri hands it to the main thread. It therefore runs on a blocking
// worker, never on the async runtime. A choice does not come back from the popup call: it is a
// `MenuEvent`, delivered through the event loop. Each call gets a token, each item an id made from
// the token and the item's place in the menu, and a `Session` registers the token for as long as the
// call lasts, so an event from an earlier menu can never answer a later one. After the popup
// returns, a task posted to the main thread acts as a barrier: events are handled in the order they
// were queued, so once it runs, the choice (if any) has been delivered.
//
// The icon size comes from the patched `muda` (see the workspace `Cargo.toml`): without it GTK draws
// every menu icon at 16 pixels whatever the screen's scale.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex, Once};
use std::time::{Duration, Instant};

use serde::Deserialize;
use tauri::image::Image;
use tauri::menu::{
    CheckMenuItem, ContextMenu, IconMenuItem, IsMenuItem, Menu, MenuEvent, MenuItem,
    PredefinedMenuItem, Submenu,
};
use tauri::{AppHandle, LogicalPosition, Manager, Runtime, State, Window};

/// Most items in one menu, counting those in submenus.
const MAX_ITEMS: usize = 200;
/// Most levels of submenu below the menu itself (Open With inside the entry menu is one).
const MAX_DEPTH: usize = 3;
/// Longest label or shortcut, in characters.
const MAX_TEXT_CHARS: usize = 256;
/// Longest page id, in characters.
const MAX_ID_CHARS: usize = 256;
/// Largest icon edge, in pixels. The page draws 16 logical pixels at the screen's scale.
const MAX_ICON_EDGE: u32 = 64;
/// Most icon bytes in one menu.
const MAX_ICON_BYTES: usize = 1 << 20;
/// Furthest a position may lie from the window's corner, in logical pixels.
const MAX_COORDINATE: f64 = 100_000.0;
/// The logical size menu icons are drawn at, which the patched `muda` scales to the screen.
const MENU_ICON_SIZE: u32 = 16;
/// A menu that closes sooner than this with no choice never really showed (tauri#13608 on GTK 3 and
/// Wayland), and is reported as a failure so the page shows its own menu instead.
const SHOWN_AT_LEAST: Duration = Duration::from_millis(40);
/// How long to wait for the main thread to have handled the events a closed menu queued.
const BARRIER_TIMEOUT: Duration = Duration::from_secs(2);
/// Only the Win32 menu draws a shortcut as right-aligned accelerator text; GTK 3 shows one only
/// for an item in an accelerator group, which a popup menu has none of.
const SHOWS_SHORTCUTS: bool = cfg!(windows);

fn yes() -> bool {
    true
}

/// A picture for an item: straight RGBA, `width` by `height` pixels.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeMenuIcon {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

/// One entry of the menu the page asks for.
#[derive(Debug, Clone, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum NativeMenuItem {
    Action {
        id: String,
        label: String,
        #[serde(default = "yes")]
        enabled: bool,
        #[serde(default)]
        shortcut: Option<String>,
        #[serde(default)]
        icon: Option<NativeMenuIcon>,
    },
    Checkbox {
        id: String,
        label: String,
        checked: bool,
        #[serde(default = "yes")]
        enabled: bool,
        #[serde(default)]
        shortcut: Option<String>,
    },
    Submenu {
        label: String,
        #[serde(default = "yes")]
        enabled: bool,
        #[serde(default)]
        icon: Option<NativeMenuIcon>,
        items: Vec<NativeMenuItem>,
    },
    Separator,
}

/// Where the menu's top-left corner goes: logical pixels from the window's top-left corner.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct MenuPoint {
    x: f64,
    y: f64,
}

/// A checked description, with every item given the id the system will report it by.
#[derive(Debug, Clone, PartialEq)]
enum Planned {
    Action {
        menu_id: String,
        text: String,
        enabled: bool,
        shortcut: Option<String>,
        icon: Option<Arc<NativeMenuIcon>>,
    },
    Checkbox {
        menu_id: String,
        text: String,
        checked: bool,
        enabled: bool,
        shortcut: Option<String>,
    },
    Submenu {
        menu_id: String,
        text: String,
        enabled: bool,
        icon: Option<Arc<NativeMenuIcon>>,
        children: Vec<Planned>,
    },
    Separator,
}

/// What `plan` makes of a description.
#[derive(Debug, PartialEq)]
struct Plan {
    entries: Vec<Planned>,
    /// The page's id of the item numbered by its index; a submenu has none.
    choices: Vec<Option<String>>,
}

/// The id the system reports an item by: the call's token and the item's number.
fn menu_id(token: u64, index: usize) -> String {
    format!("wpnm-{token}-{index}")
}

/// The token and item number in an id made by `menu_id`; `None` for any other menu's id.
fn parse_menu_id(id: &str) -> Option<(u64, usize)> {
    let rest = id.strip_prefix("wpnm-")?;
    let (token, index) = rest.split_once('-')?;
    Some((token.parse().ok()?, index.parse().ok()?))
}

/// `&` starts a mnemonic in a label, and `&&` is a literal one, so a label's own `&` is doubled.
fn escape_label(label: &str) -> String {
    label.replace('&', "&&")
}

fn check_text(what: &str, text: &str, max: usize) -> Result<(), String> {
    if text.chars().count() > max {
        return Err(format!("a {what} is longer than {max} characters"));
    }
    Ok(())
}

fn check_icon(icon: &NativeMenuIcon, icon_bytes: &mut usize) -> Result<(), String> {
    let edges = 1..=MAX_ICON_EDGE;
    if !edges.contains(&icon.width) || !edges.contains(&icon.height) {
        return Err(format!(
            "an icon is {}×{} pixels, and must be between 1 and {MAX_ICON_EDGE} on each side",
            icon.width, icon.height
        ));
    }
    if icon.rgba.len() != icon.width as usize * icon.height as usize * 4 {
        return Err("an icon's pixels do not fill its width and height".into());
    }
    *icon_bytes += icon.rgba.len();
    if *icon_bytes > MAX_ICON_BYTES {
        return Err(format!("the icons are larger than {MAX_ICON_BYTES} bytes"));
    }
    Ok(())
}

/// Checks `items` against the limits and numbers them for `token`.
fn plan(items: &[NativeMenuItem], token: u64) -> Result<Plan, String> {
    struct Walk {
        token: u64,
        choices: Vec<Option<String>>,
        icon_bytes: usize,
    }
    fn walk(
        items: &[NativeMenuItem],
        depth: usize,
        state: &mut Walk,
    ) -> Result<Vec<Planned>, String> {
        if depth > MAX_DEPTH {
            return Err(format!("submenus are nested more than {MAX_DEPTH} deep"));
        }
        let mut planned = Vec::with_capacity(items.len());
        for item in items {
            if state.choices.len() >= MAX_ITEMS {
                return Err(format!("the menu has more than {MAX_ITEMS} items"));
            }
            let index = state.choices.len();
            let menu_id = menu_id(state.token, index);
            let shortcut_of = |shortcut: &Option<String>| -> Result<Option<String>, String> {
                match shortcut {
                    Some(text) => {
                        check_text("shortcut", text, MAX_TEXT_CHARS)?;
                        Ok(SHOWS_SHORTCUTS
                            .then(|| text.trim().to_owned())
                            .filter(|s| !s.is_empty()))
                    }
                    None => Ok(None),
                }
            };
            match item {
                NativeMenuItem::Action {
                    id,
                    label,
                    enabled,
                    shortcut,
                    icon,
                } => {
                    check_text("id", id, MAX_ID_CHARS)?;
                    check_text("label", label, MAX_TEXT_CHARS)?;
                    if let Some(icon) = icon {
                        check_icon(icon, &mut state.icon_bytes)?;
                    }
                    state.choices.push(Some(id.clone()));
                    planned.push(Planned::Action {
                        menu_id,
                        text: escape_label(label),
                        enabled: *enabled,
                        shortcut: shortcut_of(shortcut)?,
                        icon: icon.clone().map(Arc::new),
                    });
                }
                NativeMenuItem::Checkbox {
                    id,
                    label,
                    checked,
                    enabled,
                    shortcut,
                } => {
                    check_text("id", id, MAX_ID_CHARS)?;
                    check_text("label", label, MAX_TEXT_CHARS)?;
                    state.choices.push(Some(id.clone()));
                    planned.push(Planned::Checkbox {
                        menu_id,
                        text: escape_label(label),
                        checked: *checked,
                        enabled: *enabled,
                        shortcut: shortcut_of(shortcut)?,
                    });
                }
                NativeMenuItem::Submenu {
                    label,
                    enabled,
                    icon,
                    items,
                } => {
                    check_text("label", label, MAX_TEXT_CHARS)?;
                    if let Some(icon) = icon {
                        check_icon(icon, &mut state.icon_bytes)?;
                    }
                    state.choices.push(None);
                    let children = walk(items, depth + 1, state)?;
                    planned.push(Planned::Submenu {
                        menu_id,
                        text: escape_label(label),
                        enabled: *enabled,
                        icon: icon.clone().map(Arc::new),
                        children,
                    });
                }
                NativeMenuItem::Separator => planned.push(Planned::Separator),
            }
        }
        Ok(planned)
    }

    let mut state = Walk {
        token,
        choices: Vec::new(),
        icon_bytes: 0,
    };
    let entries = walk(items, 0, &mut state)?;
    if state.choices.iter().all(Option::is_none) {
        return Err("the menu has nothing to choose".into());
    }
    Ok(Plan {
        entries,
        choices: state.choices,
    })
}

/// A position inside the limits, as the logical position the popup takes.
fn position_of(at: MenuPoint) -> Result<LogicalPosition<f64>, String> {
    let sane = |v: f64| v.is_finite() && v.abs() <= MAX_COORDINATE;
    if !sane(at.x) || !sane(at.y) {
        return Err("the menu's position is not a point in the window".into());
    }
    Ok(LogicalPosition::new(at.x, at.y))
}

/// Only a main window shows native menus: they are what the file browser opens them for.
fn may_show(label: &str) -> bool {
    label.starts_with("main-")
}

#[derive(Default)]
struct Shared {
    next_token: AtomicU64,
    busy: AtomicBool,
    pending: Mutex<HashMap<u64, Sender<usize>>>,
}

/// The menus being shown: which call each reported item belongs to, and that only one is up at a time.
#[derive(Clone, Default)]
pub struct NativeMenus(Arc<Shared>);

/// One call's claim on the registry, released when it is dropped.
struct Session {
    menus: NativeMenus,
    token: u64,
    chosen: std::sync::mpsc::Receiver<usize>,
}

impl NativeMenus {
    /// Claims the registry for a call, or `None` while another menu is up.
    fn begin(&self) -> Option<Session> {
        if self.0.busy.swap(true, Ordering::AcqRel) {
            return None;
        }
        let token = self.0.next_token.fetch_add(1, Ordering::Relaxed);
        let (sender, chosen) = channel();
        self.0
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(token, sender);
        Some(Session {
            menus: self.clone(),
            token,
            chosen,
        })
    }

    /// Hands a reported item to the call it belongs to; `false` when that call is over.
    fn deliver(&self, token: u64, index: usize) -> bool {
        let pending = self.0.pending.lock().unwrap_or_else(|e| e.into_inner());
        pending
            .get(&token)
            .is_some_and(|sender| sender.send(index).is_ok())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.menus
            .0
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.token);
        self.menus.0.busy.store(false, Ordering::Release);
    }
}

fn build_item<R: Runtime, M: Manager<R>>(
    manager: &M,
    planned: &Planned,
) -> tauri::Result<Box<dyn IsMenuItem<R>>> {
    Ok(match planned {
        Planned::Action {
            menu_id,
            text,
            enabled,
            shortcut,
            icon,
        } => match icon {
            Some(icon) => Box::new(IconMenuItem::with_id(
                manager,
                menu_id.as_str(),
                text,
                *enabled,
                Some(Image::new_owned(icon.rgba.clone(), icon.width, icon.height)),
                shortcut.as_deref(),
            )?),
            None => Box::new(MenuItem::with_id(
                manager,
                menu_id.as_str(),
                text,
                *enabled,
                shortcut.as_deref(),
            )?),
        },
        Planned::Checkbox {
            menu_id,
            text,
            checked,
            enabled,
            shortcut,
        } => Box::new(CheckMenuItem::with_id(
            manager,
            menu_id.as_str(),
            text,
            *enabled,
            *checked,
            shortcut.as_deref(),
        )?),
        Planned::Submenu {
            menu_id,
            text,
            enabled,
            icon,
            children,
        } => {
            let submenu = match icon {
                Some(icon) => Submenu::with_id_and_icon(
                    manager,
                    menu_id.as_str(),
                    text,
                    *enabled,
                    Some(Image::new_owned(icon.rgba.clone(), icon.width, icon.height)),
                )?,
                None => Submenu::with_id(manager, menu_id.as_str(), text, *enabled)?,
            };
            for child in children {
                submenu.append(build_item(manager, child)?.as_ref())?;
            }
            Box::new(submenu)
        }
        Planned::Separator => Box::new(PredefinedMenuItem::separator(manager)?),
    })
}

fn build_menu<R: Runtime, M: Manager<R>>(
    manager: &M,
    entries: &[Planned],
) -> tauri::Result<Menu<R>> {
    let menu = Menu::new(manager)?;
    for entry in entries {
        menu.append(build_item(manager, entry)?.as_ref())?;
    }
    Ok(menu)
}

/// Builds the menu, pops it up and waits for it to close. Runs on a blocking worker.
fn show<R: Runtime>(
    window: Window<R>,
    session: Session,
    plan: Plan,
    at: Option<LogicalPosition<f64>>,
) -> Result<Option<String>, String> {
    static ICON_SIZE: Once = Once::new();
    ICON_SIZE.call_once(|| muda::set_default_menu_icon_size(Some(MENU_ICON_SIZE)));

    // The menu stays alive until the barrier below: dropping it early can lose its events (tauri#9470).
    let menu = build_menu(&window, &plan.entries).map_err(|e| e.to_string())?;
    let opened = Instant::now();
    let shown = match at {
        Some(at) => menu.popup_at(window.clone(), at),
        None => menu.popup(window.clone()),
    };
    let lasted = opened.elapsed();
    shown.map_err(|e| e.to_string())?;

    let (reached, ack) = channel();
    window
        .app_handle()
        .run_on_main_thread(move || {
            let _ = reached.send(());
        })
        .map_err(|e| e.to_string())?;
    if ack.recv_timeout(BARRIER_TIMEOUT).is_err() {
        log::warn!("native menu: the main thread did not catch up after the menu closed");
    }
    let chosen = session.chosen.try_recv().ok();
    drop(menu);

    match chosen {
        Some(index) => Ok(plan.choices.get(index).cloned().flatten()),
        None if lasted < SHOWN_AT_LEAST => Err(format!(
            "the system menu closed after {} ms without a choice",
            lasted.as_millis()
        )),
        None => Ok(None),
    }
}

/// Shows `items` as the system's menu on the calling main window and returns the chosen item's id,
/// or `None` if the menu was dismissed (or another was already up). An error means no menu was shown
/// that the person could use, and the page should show its own.
#[tauri::command]
pub async fn show_native_menu<R: Runtime>(
    window: Window<R>,
    menus: State<'_, NativeMenus>,
    items: Vec<NativeMenuItem>,
    at: Option<MenuPoint>,
) -> Result<Option<String>, String> {
    if !may_show(window.label()) {
        return Err(format!("`{}` is not a main window", window.label()));
    }
    let at = at.map(position_of).transpose()?;
    let Some(session) = menus.begin() else {
        log::debug!("native menu: one is already open, so this call is dropped");
        return Ok(None);
    };
    let plan = plan(&items, session.token)?;
    tauri::async_runtime::spawn_blocking(move || show(window, session, plan, at))
        .await
        .map_err(|e| e.to_string())?
}

/// Starts listening for chosen items. Call once from `setup`.
pub fn wire<R: Runtime>(app: &AppHandle<R>) {
    let menus = NativeMenus::default();
    app.manage(menus.clone());
    app.on_menu_event(move |_, event: MenuEvent| {
        if let Some((token, index)) = parse_menu_id(event.id().as_ref()) {
            if !menus.deliver(token, index) {
                log::debug!("native menu: a choice arrived after its menu was over");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action(id: &str, label: &str) -> NativeMenuItem {
        NativeMenuItem::Action {
            id: id.into(),
            label: label.into(),
            enabled: true,
            shortcut: None,
            icon: None,
        }
    }

    fn icon(edge: u32) -> NativeMenuIcon {
        NativeMenuIcon {
            width: edge,
            height: edge,
            rgba: vec![255; (edge * edge * 4) as usize],
        }
    }

    #[test]
    fn a_description_is_read_from_the_pages_json() {
        let items: Vec<NativeMenuItem> = serde_json::from_str(
            r#"[
                {"kind":"action","id":"open","label":"Open","shortcut":"Enter",
                 "icon":{"width":1,"height":1,"rgba":[1,2,3,4]}},
                {"kind":"separator"},
                {"kind":"checkbox","id":"hidden","label":"Hidden","checked":true,"enabled":false},
                {"kind":"submenu","label":"Sort by","items":[{"kind":"action","id":"name","label":"Name"}]}
            ]"#,
        )
        .unwrap();
        assert_eq!(items.len(), 4);
        let planned = plan(&items, 1).unwrap();
        assert_eq!(planned.choices.len(), 4);
    }

    #[test]
    fn an_unknown_kind_is_refused_when_read() {
        assert!(
            serde_json::from_str::<NativeMenuItem>(r#"{"kind":"radio","id":"a","label":"A"}"#)
                .is_err()
        );
    }

    #[test]
    fn items_are_numbered_in_order_through_the_submenus_and_a_submenu_is_not_a_choice() {
        let items = [
            action("open", "Open"),
            NativeMenuItem::Separator,
            NativeMenuItem::Submenu {
                label: "More".into(),
                enabled: true,
                icon: None,
                items: vec![action("copy", "Copy"), action("cut", "Cut")],
            },
            action("last", "Last"),
        ];
        let planned = plan(&items, 7).unwrap();
        assert_eq!(
            planned.choices,
            [
                Some("open".to_owned()),
                None,
                Some("copy".to_owned()),
                Some("cut".to_owned()),
                Some("last".to_owned())
            ]
        );
        let Planned::Submenu {
            menu_id, children, ..
        } = &planned.entries[2]
        else {
            panic!("the third entry is the submenu");
        };
        assert_eq!(menu_id, "wpnm-7-1");
        assert!(matches!(&children[1], Planned::Action { menu_id, .. } if menu_id == "wpnm-7-3"));
    }

    #[test]
    fn an_item_id_names_its_call_and_place_and_nothing_else_parses() {
        assert_eq!(parse_menu_id(&menu_id(12, 3)), Some((12, 3)));
        for other in [
            "", "open", "wpnm-", "wpnm-1", "wpnm-a-1", "wpnm-1-b", "app-1-2",
        ] {
            assert_eq!(parse_menu_id(other), None, "{other}");
        }
    }

    #[test]
    fn an_ampersand_in_a_label_is_shown_as_one() {
        let planned = plan(&[action("a", "Sort & Group")], 0).unwrap();
        let Planned::Action { text, .. } = &planned.entries[0] else {
            panic!("an action");
        };
        assert_eq!(text, "Sort && Group");
    }

    #[test]
    fn a_shortcut_is_kept_only_where_the_system_menu_draws_it() {
        let items = [NativeMenuItem::Action {
            id: "copy".into(),
            label: "Copy".into(),
            enabled: true,
            shortcut: Some(" Ctrl+C ".into()),
            icon: None,
        }];
        let planned = plan(&items, 0).unwrap();
        let Planned::Action { shortcut, .. } = &planned.entries[0] else {
            panic!("an action");
        };
        assert_eq!(shortcut.as_deref(), SHOWS_SHORTCUTS.then_some("Ctrl+C"));
    }

    #[test]
    fn a_menu_with_nothing_to_choose_is_refused() {
        assert!(plan(&[], 0).is_err());
        assert!(plan(&[NativeMenuItem::Separator], 0).is_err());
    }

    #[test]
    fn too_many_items_are_refused_counting_those_in_submenus() {
        let many: Vec<_> = (0..MAX_ITEMS)
            .map(|n| action(&n.to_string(), "Item"))
            .collect();
        assert!(plan(&many, 0).is_ok());
        let mut over = many.clone();
        over.push(action("one-more", "Item"));
        assert!(plan(&over, 0).is_err());
        let nested = [NativeMenuItem::Submenu {
            label: "More".into(),
            enabled: true,
            icon: None,
            items: many,
        }];
        assert!(plan(&nested, 0).is_err(), "the submenu itself is one more");
    }

    #[test]
    fn submenus_nested_too_deep_are_refused() {
        let mut items = vec![action("leaf", "Leaf")];
        for level in 0..MAX_DEPTH {
            items = vec![NativeMenuItem::Submenu {
                label: format!("Level {level}"),
                enabled: true,
                icon: None,
                items,
            }];
        }
        assert!(plan(&items, 0).is_ok());
        items = vec![NativeMenuItem::Submenu {
            label: "One too many".into(),
            enabled: true,
            icon: None,
            items,
        }];
        assert!(plan(&items, 0).is_err());
    }

    #[test]
    fn long_labels_ids_and_shortcuts_are_refused() {
        let long = "x".repeat(MAX_TEXT_CHARS + 1);
        assert!(plan(&[action("a", &long)], 0).is_err());
        assert!(plan(&[action(&"y".repeat(MAX_ID_CHARS + 1), "A")], 0).is_err());
        let shortcut = NativeMenuItem::Checkbox {
            id: "c".into(),
            label: "C".into(),
            checked: false,
            enabled: true,
            shortcut: Some(long),
        };
        assert!(plan(&[shortcut], 0).is_err());
        assert!(plan(&[action("a", &"é".repeat(MAX_TEXT_CHARS))], 0).is_ok());
    }

    #[test]
    fn an_icon_must_fill_its_size_and_stay_small() {
        let with = |icon| NativeMenuItem::Action {
            id: "a".into(),
            label: "A".into(),
            enabled: true,
            shortcut: None,
            icon: Some(icon),
        };
        assert!(plan(&[with(icon(32))], 0).is_ok());
        assert!(plan(&[with(icon(MAX_ICON_EDGE + 1))], 0).is_err());
        assert!(plan(
            &[with(NativeMenuIcon {
                width: 0,
                height: 0,
                rgba: vec![]
            })],
            0
        )
        .is_err());
        let mut short = icon(16);
        short.rgba.pop();
        assert!(plan(&[with(short)], 0).is_err());
        let mut wrong = icon(16);
        wrong.width = 17;
        assert!(plan(&[with(wrong)], 0).is_err());
    }

    #[test]
    fn a_submenu_icon_is_checked_and_counted_like_an_actions() {
        let with = |icon| {
            [NativeMenuItem::Submenu {
                label: "More".into(),
                enabled: true,
                icon: Some(icon),
                items: vec![action("a", "A")],
            }]
        };
        let planned = plan(&with(icon(16)), 0).unwrap();
        assert!(matches!(
            &planned.entries[0],
            Planned::Submenu { icon: Some(icon), .. } if icon.width == 16
        ));
        assert!(plan(&with(icon(MAX_ICON_EDGE + 1)), 0).is_err());
        let mut short = icon(16);
        short.rgba.pop();
        assert!(plan(&with(short), 0).is_err());
        let fits = MAX_ICON_BYTES / (MAX_ICON_EDGE * MAX_ICON_EDGE * 4) as usize;
        let mut items: Vec<_> = (0..fits)
            .map(|n| NativeMenuItem::Action {
                id: n.to_string(),
                label: "A".into(),
                enabled: true,
                shortcut: None,
                icon: Some(icon(MAX_ICON_EDGE)),
            })
            .collect();
        items.extend(with(icon(MAX_ICON_EDGE)));
        assert!(
            plan(&items, 0).is_err(),
            "the submenu's icon is in the budget"
        );
    }

    #[test]
    fn the_icons_together_have_a_byte_budget() {
        let big = |n: usize| NativeMenuItem::Action {
            id: n.to_string(),
            label: "A".into(),
            enabled: true,
            shortcut: None,
            icon: Some(icon(MAX_ICON_EDGE)),
        };
        let fits = MAX_ICON_BYTES / (MAX_ICON_EDGE * MAX_ICON_EDGE * 4) as usize;
        let ok: Vec<_> = (0..fits).map(big).collect();
        assert!(plan(&ok, 0).is_ok());
        let over: Vec<_> = (0..=fits).map(big).collect();
        assert!(plan(&over, 0).is_err());
    }

    #[test]
    fn a_position_must_be_a_point_near_the_window() {
        assert!(position_of(MenuPoint { x: 12.5, y: 40.0 }).is_ok());
        assert!(position_of(MenuPoint { x: -3.0, y: 0.0 }).is_ok());
        assert!(position_of(MenuPoint {
            x: f64::NAN,
            y: 0.0
        })
        .is_err());
        assert!(position_of(MenuPoint {
            x: 0.0,
            y: f64::INFINITY
        })
        .is_err());
        assert!(position_of(MenuPoint {
            x: MAX_COORDINATE + 1.0,
            y: 0.0
        })
        .is_err());
    }

    #[test]
    fn only_a_main_window_may_show_one() {
        assert!(may_show("main-1"));
        assert!(may_show("main-42"));
        for label in [
            "settings",
            "shelf",
            "ops",
            "properties-1",
            "tear-ghost",
            "main",
            "xmain-1",
        ] {
            assert!(!may_show(label), "{label}");
        }
    }

    #[test]
    fn a_choice_reaches_only_the_call_that_is_open() {
        let menus = NativeMenus::default();
        let first = menus.begin().expect("nothing else is open");
        let token = first.token;
        assert!(menus.begin().is_none(), "one menu at a time");
        assert!(menus.deliver(token, 4));
        assert_eq!(first.chosen.try_recv(), Ok(4));
        drop(first);

        // A late event of the first menu finds nobody, and the next call has a token of its own.
        assert!(!menus.deliver(token, 5));
        let second = menus.begin().expect("the first is over");
        assert_ne!(second.token, token);
        assert!(!menus.deliver(token, 6));
        assert!(second.chosen.try_recv().is_err());
        assert!(menus.deliver(second.token, 1));
        assert_eq!(second.chosen.try_recv(), Ok(1));
    }
}
