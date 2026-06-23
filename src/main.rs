//! grun — a small GTK4 application launcher inspired by KDE's KRunner.
//!
//! Architecture: a `Registry` holds enabled `Provider`s. As the user types,
//! every provider is queried, results are merged + scored, and shown in a list.
//! Enter runs the selected match's `Action`; Esc quits. A gear button opens a
//! settings window to toggle providers.

mod config;
mod matching;
mod providers;
mod state;
mod text;

use gtk4::prelude::*;
use gtk4::{
    gdk, glib, Align, Application, ApplicationWindow, Box as GtkBox, Button, Entry,
    EventControllerKey, Image, Label, Orientation, PolicyType, ScrolledWindow, Switch, Window,
};
use std::cell::RefCell;
use std::rc::Rc;

use config::Config;
use matching::{Action, Match};
use providers::{app_to_match, file_to_match, Registry};
use state::{ClipEntry, History};
use std::collections::HashMap;

const APP_ID: &str = "org.grun.Launcher";
const WIN_W: i32 = 860;
const WIN_H: i32 = 720;
/// Tallest the scrollable result area grows before it starts scrolling. Leaves
/// room for the search bar so the whole window peaks near WIN_H.
const CONTENT_MAX_H: i32 = WIN_H - 80;
/// Home dashboard cards per row (one row collapsed, two when expanded).
const HOME_ROW: usize = 3;
/// Cards per row in the full-screen app grid.
const HOME_ROW_FULL: usize = 6;
/// Fixed home-card size, so cards form a uniform grid regardless of how many
/// side-action buttons a card has (Flatpak apps have an extra "Uninstall", etc.)
/// and wrap into rows instead of stretching to fill (which broke the layout).
const CARD_W: i32 = 270;
const CARD_H: i32 = 176;

// Assets embedded at build time, so the binary is self-contained (no dependency
// on the source tree at runtime — important for a packaged install).
// Section icons come in dark (light-coloured) and light (dark-coloured) variants
// so they read well on either system theme. Both sets are embedded and written
// to disk on first run; `section_icon` picks the right one per the active theme.
const EMB_CLIPBOARD: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/Assets/Dark-Clipboard.svg"));
const EMB_APPS: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/Assets/Dark-apps.svg"));
const EMB_FILES: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/Assets/Dark-Files.svg"));
const EMB_SEARCH: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/Assets/Dark-search.svg"));
const EMB_AI: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/Assets/Dark-Ai.svg"));
const EMB_CLIPBOARD_LIGHT: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/Assets/Light-Clipboard.svg"));
const EMB_APPS_LIGHT: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/Assets/Light-apps.svg"));
const EMB_FILES_LIGHT: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/Assets/Light-Files.svg"));
const EMB_SEARCH_LIGHT: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/Assets/Light-search.svg"));
const EMB_AI_LIGHT: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/Assets/Light-AI.svg"));
// Expand / collapse arrows for the home section headers.
const EMB_DOWN: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/Assets/Dark-pointer-Down.svg"));
const EMB_UP: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/Assets/Dark-pointer-up.svg"));
const EMB_DOWN_LIGHT: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/Assets/Light-pointer-Down.svg"));
const EMB_UP_LIGHT: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/Assets/Light-pointer-up.svg"));
const EMB_APPICON_256: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/Assets/AppIcon-256.png"));
const EMB_APPICON_512: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/Assets/AppIcon-512.png"));

// Colours are taken from the running GTK theme so grun follows the system
// light/dark theme and its Mint-Y accent automatically:
//   @theme_bg_color / @theme_fg_color  — window background / foreground
//   @theme_selected_bg_color / _fg     — the accent (selection) colour
// Surfaces are solid (Mint-Y has no transparency by default); `alpha()` is only
// used for subtle borders and muted text that composite over those solid colours.
const CSS: &str = r#"
window {
    background-color: @theme_bg_color;
    color: @theme_fg_color;
    border: 1px solid alpha(@theme_fg_color, 0.18);
    border-radius: 12px;
}
.grun-entry {
    font-size: 20px;
    padding: 16px 18px;
    border: none;
    background: transparent;
    color: @theme_fg_color;
    box-shadow: none;
    outline: none;
}
/* The search bar (entry + gear). It glows with the accent colour while the
   entry has focus, so on open the focus is clearly on the search box. */
.grun-searchbar {
    border: 1px solid transparent;
    border-radius: 12px;
    margin: 6px;
}
.grun-searchbar.focused {
    border-color: @theme_selected_bg_color;
    box-shadow: 0 0 6px 0 alpha(@theme_selected_bg_color, 0.55);
}
.grun-gear {
    background: transparent;
    border: none;
    box-shadow: none;
    margin: 6px;
    color: alpha(@theme_fg_color, 0.6);
}
.grun-gear:hover { color: @theme_fg_color; }
.grun-row {
    padding: 8px 12px;
    margin: 1px 6px;
    border-radius: 10px;
}
.grun-row.selected { background-color: mix(@theme_bg_color, @theme_selected_bg_color, 0.35); }
.grun-title { color: @theme_fg_color; font-size: 14px; font-weight: bold; }
.grun-sub { color: alpha(@theme_fg_color, 0.55); font-size: 12px; }
.grun-section {
    color: alpha(@theme_fg_color, 0.5);
    font-size: 11px;
    font-weight: bold;
    text-transform: uppercase;
    letter-spacing: 1px;
}
.grun-letter {
    color: @theme_fg_color;
    background-color: alpha(@theme_fg_color, 0.12);
    border-radius: 6px;
    padding: 1px 7px;
    font-weight: bold;
    font-size: 13px;
    min-width: 14px;
}
.grun-chip {
    background-color: alpha(@theme_fg_color, 0.10);
    color: alpha(@theme_fg_color, 0.85);
    border: none;
    box-shadow: none;
    border-radius: 8px;
    padding: 2px 8px;
    font-size: 11px;
    min-height: 0;
}
.grun-chip:hover { background-color: alpha(@theme_selected_bg_color, 0.30); color: @theme_fg_color; }
.grun-tag {
    color: #ffffff;
    background-color: @theme_selected_bg_color;
    border-radius: 5px;
    /* Enough vertical padding that descenders (p, g) clear the background. */
    padding: 2px 8px;
    font-size: 10px;
    font-weight: bold;
}
/* Package-type colours: deb/system red, AppImage blue, Flatpak green, Snap orange. */
.grun-tag-system { background-color: #d64541; }
.grun-tag-appimage { background-color: #2f80ed; }
.grun-tag-flatpak { background-color: #27ae60; }
.grun-tag-snap { background-color: #e67e22; }
.grun-card {
    padding: 16px;
    margin: 0;
    border-radius: 10px;
    background-color: @theme_base_color;
    border: 1px solid alpha(@theme_fg_color, 0.12);
}
.grun-card.selected {
    background-color: mix(@theme_base_color, @theme_selected_bg_color, 0.45);
    border-color: @theme_selected_bg_color;
}
.grun-card-title { color: @theme_fg_color; font-size: 15px; font-weight: bold; }
.grun-side {
    background-color: alpha(@theme_fg_color, 0.10);
    color: alpha(@theme_fg_color, 0.85);
    border: none;
    box-shadow: none;
    border-radius: 8px;
    padding: 5px 8px;
    font-size: 11px;
    min-height: 0;
}
.grun-side:hover { background-color: alpha(@theme_selected_bg_color, 0.30); color: @theme_fg_color; }
.grun-card-letter {
    background-color: alpha(@theme_selected_bg_color, 0.22);
    color: @theme_fg_color;
    border-radius: 8px;
    padding: 8px;
    font-size: 15px;
    font-weight: bold;
}
.grun-settings-title { font-size: 18px; font-weight: bold; margin-bottom: 8px; }
.grun-choice.active {
    background-color: @theme_selected_bg_color;
    color: @theme_selected_fg_color;
    font-weight: bold;
}
.grun-save {
    background-color: @theme_selected_bg_color;
    color: @theme_selected_fg_color;
    font-weight: bold;
    padding: 8px;
    border-radius: 8px;
}
.grun-save:hover { background-color: alpha(@theme_selected_bg_color, 0.85); }
"#;

fn main() -> glib::ExitCode {
    install_app_icon();
    install_desktop_entry();
    ensure_assets();
    let app = Application::builder().application_id(APP_ID).build();

    // grun runs resident: the window is built once and only hidden, never
    // destroyed. GApplication is single-instance, so binding `grun` to a global
    // shortcut means each launch re-fires `activate` on the running instance —
    // which we treat as a toggle (show if hidden, hide if visible).
    let toggle: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    app.connect_activate(move |app| {
        let existing = toggle.borrow().clone();
        let t = match existing {
            Some(t) => t,
            None => {
                load_css();
                let t = build_app(app);
                *toggle.borrow_mut() = Some(t.clone());
                t
            }
        };
        t();
    });

    app.run()
}

/// Build the launcher window (hidden) and return a closure that toggles it.
fn build_app(app: &Application) -> Rc<dyn Fn()> {
    let cfg = Rc::new(RefCell::new(Config::load()));
    let history = Rc::new(RefCell::new(History::load()));
    let registry = Rc::new(RefCell::new(Registry::from_config(&cfg.borrow(), &history)));
    // Index of installed apps by id, for resolving the "Top apps" history.
    let app_index: Rc<HashMap<String, gtk4::gio::AppInfo>> = Rc::new(
        gtk4::gio::AppInfo::all()
            .into_iter()
            .filter(gtk4::prelude::AppInfoExt::should_show)
            .filter_map(|a| a.id().map(|id| (id.to_string(), a)))
            .collect(),
    );
    // The matches currently shown, kept in sync with the list rows by index.
    let matches: Rc<RefCell<Vec<Match>>> = Rc::new(RefCell::new(Vec::new()));

    // Clipboard daemon: listen for clipboard changes and read them with GTK's
    // ASYNC API. (A blocking `xclip -o` read here would deadlock whenever grun
    // itself owns the clipboard — e.g. right after "Copy path".)
    if let Some(display) = gdk::Display::default() {
        let clipboard = display.clipboard();
        let history = history.clone();
        let last: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
        clipboard.connect_changed(move |cb| {
            if cb.formats().contain_mime_type("image/png") {
                let history = history.clone();
                let last = last.clone();
                cb.read_texture_async(gtk4::gio::Cancellable::NONE, move |res| {
                    if let Ok(Some(tex)) = res {
                        let bytes = tex.save_to_png_bytes();
                        let slice: &[u8] = &bytes;
                        let id = state::hash_bytes(slice);
                        if *last.borrow() != id {
                            *last.borrow_mut() = id.clone();
                            if let Some(dir) = state::clips_dir() {
                                let path = dir.join(format!("{id}.png"));
                                if !path.exists() {
                                    let _ = std::fs::write(&path, slice);
                                }
                                history
                                    .borrow_mut()
                                    .add_clip("image", "", &path.to_string_lossy());
                                history.borrow().save();
                            }
                        }
                    }
                });
            } else {
                let history = history.clone();
                let last = last.clone();
                cb.read_text_async(gtk4::gio::Cancellable::NONE, move |res| {
                    if let Ok(Some(text)) = res {
                        let t = text.to_string();
                        if !t.trim().is_empty() && *last.borrow() != t {
                            *last.borrow_mut() = t.clone();
                            history.borrow_mut().add_clip("text", &t, "");
                            history.borrow().save();
                        }
                    }
                });
            }
        });
    }

    let entry = Entry::builder()
        .placeholder_text("Search for everything by typing…")
        .hexpand(true)
        .build();
    entry.add_css_class("grun-entry");

    let full_btn = Button::from_icon_name(if cfg.borrow().fullscreen {
        "view-restore-symbolic"
    } else {
        "view-fullscreen-symbolic"
    });
    full_btn.add_css_class("grun-gear");
    full_btn.set_valign(Align::Center);
    full_btn.set_tooltip_text(Some("Full-screen start-menu home"));

    let gear = Button::from_icon_name("emblem-system-symbolic");
    gear.add_css_class("grun-gear");
    gear.set_valign(Align::Center);

    let top = GtkBox::new(Orientation::Horizontal, 0);
    top.add_css_class("grun-searchbar");
    top.append(&entry);
    top.append(&full_btn);
    top.append(&gear);

    // Results live in a plain vertical box so we control grouping, per-row
    // letters, action chips, and highlight directly.
    let results_box = GtkBox::new(Orientation::Vertical, 2);

    // The window sizes to its content: it requests the result list's natural
    // height (so an empty home is just the search box) and grows as results come
    // in, capped at CONTENT_MAX_H — beyond that the list scrolls. This makes the
    // window shrink when categories are disabled or items are hidden.
    let scroller = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .propagate_natural_height(true)
        .min_content_height(0)
        .max_content_height(CONTENT_MAX_H)
        .child(&results_box)
        .build();

    let vbox = GtkBox::new(Orientation::Vertical, 0);
    vbox.append(&top);
    vbox.append(&scroller);

    let window = ApplicationWindow::builder()
        .application(app)
        .title("grunlauncher")
        .default_width(WIN_W)
        .decorated(false)
        .resizable(false)
        .child(&vbox)
        .build();

    // Hide (don't destroy) when the window is closed, so the app stays resident.
    window.connect_close_request(|w| {
        w.set_visible(false);
        glib::Propagation::Stop
    });

    // --- navigation state ---
    let rows: Rc<RefCell<Vec<gtk4::Widget>>> = Rc::new(RefCell::new(Vec::new()));
    let selected = Rc::new(RefCell::new(0usize));
    let nav_mode = Rc::new(RefCell::new(false));
    // Per-section "expand" state on the home dashboard (category → showing 2 rows).
    // Runtime only; resets to collapsed each time grun is opened.
    let expanded: Rc<RefCell<HashMap<String, bool>>> = Rc::new(RefCell::new(HashMap::new()));
    // Generation counter so each keystroke cancels the previous auto-focus timer.
    let focus_gen = Rc::new(std::cell::Cell::new(0u64));
    let refresh_slot: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));

    // Glow the search bar only while in search mode (typing), not while the user
    // is navigating the list below — where the keyboard drives row selection and
    // typing is disabled, so a glow would wrongly imply you can write.
    let set_search_glow: Rc<dyn Fn(bool)> = {
        let top = top.clone();
        Rc::new(move |active: bool| {
            if active {
                top.add_css_class("focused");
            } else {
                top.remove_css_class("focused");
            }
        })
    };

    // Highlight row `idx`.
    let select: Rc<dyn Fn(usize)> = {
        let rows = rows.clone();
        let selected = selected.clone();
        Rc::new(move |idx: usize| {
            let rows = rows.borrow();
            if rows.is_empty() {
                return;
            }
            let idx = idx.min(rows.len() - 1);
            for (i, w) in rows.iter().enumerate() {
                if i == idx {
                    w.add_css_class("selected");
                } else {
                    w.remove_css_class("selected");
                }
            }
            *selected.borrow_mut() = idx;
        })
    };

    // Run the primary action of row `idx`.
    let run_primary: Rc<dyn Fn(usize)> = {
        let matches = matches.clone();
        let history = history.clone();
        let window = window.clone();
        let refresh_slot = refresh_slot.clone();
        Rc::new(move |idx: usize| {
            let close = {
                let ms = matches.borrow();
                match ms.get(idx) {
                    Some(m) => perform(&m.action, &history),
                    None => return,
                }
            };
            if close {
                window.set_visible(false);
            } else if let Some(r) = refresh_slot.borrow().clone() {
                r();
            }
        })
    };

    // Run the `n`-th secondary action of row `idx`.
    let run_secondary: Rc<dyn Fn(usize, usize)> = {
        let matches = matches.clone();
        let history = history.clone();
        let window = window.clone();
        let refresh_slot = refresh_slot.clone();
        Rc::new(move |idx: usize, n: usize| {
            let close = {
                let ms = matches.borrow();
                match ms.get(idx).and_then(|m| m.actions.get(n)) {
                    Some((_, _, act)) => perform(act, &history),
                    None => return,
                }
            };
            if close {
                window.set_visible(false);
            } else if let Some(r) = refresh_slot.borrow().clone() {
                r();
            }
        })
    };

    // Rebuild the grouped result list. Empty query → recent clipboard; otherwise
    // ranked results, grouped by category with per-row letters and action chips.
    let refresh: Rc<dyn Fn()> = {
        let entry = entry.clone();
        let results_box = results_box.clone();
        let registry = registry.clone();
        let history = history.clone();
        let matches = matches.clone();
        let rows = rows.clone();
        let select = select.clone();
        let run_primary = run_primary.clone();
        let run_secondary = run_secondary.clone();
        let app_index = app_index.clone();
        let cfg = cfg.clone();
        let nav_mode = nav_mode.clone();
        let selected = selected.clone();
        let expanded = expanded.clone();
        let refresh_slot = refresh_slot.clone();
        Rc::new(move || {
            let query = entry.text();
            let results: Vec<Match> = if query.trim().is_empty() {
                // Default dashboard, ordered per settings. Each section shows one
                // row (HOME_ROW), or two when "expanded". Full-screen mode is a
                // start menu sized to the screen: one clipboard row on top, one
                // files row at the bottom, and as many app rows as fit between.
                let c = cfg.borrow();
                let h = history.borrow();
                let fs = c.fullscreen;
                let exp = |cat: &str| expanded.borrow().get(cat).copied().unwrap_or(false);
                let cap = |cat: &str| if exp(cat) { HOME_ROW * 2 } else { HOME_ROW };
                // How many app rows fit on screen between the clipboard and files
                // rows (full screen only).
                let app_rows_fit = || {
                    let (_, _, _, mh) = monitor_geometry();
                    let row_h = CARD_H + 12; // card + row spacing/margin
                    // search bar + clipboard (header + row) + files (header + row)
                    // + apps header + outer padding.
                    let reserved = 80 + 2 * (30 + row_h) + 30 + 60;
                    (((mh - reserved) / row_h).max(1)) as usize
                };
                let mut v = Vec::new();
                if c.home_clipboard {
                    let n = if fs { HOME_ROW_FULL } else { cap("Clipboard") };
                    v.extend(h.visible_clips().into_iter().take(n).map(clip_to_match));
                }
                // Pull extra candidates so dropping home-hidden ones still fills
                // the row. Each card gets a "Hide" action that removes it from the
                // home dashboard only (it stays searchable).
                let app_ids: Vec<String> = if fs {
                    // Start menu: most-used apps first, enough to fill the rows
                    // that fit on screen.
                    let want = app_rows_fit() * HOME_ROW_FULL;
                    let mut ids: Vec<String> = app_index.keys().cloned().collect();
                    ids.sort_by(|a, b| {
                        h.app_count(b)
                            .cmp(&h.app_count(a))
                            .then_with(|| app_index[a].display_name().cmp(&app_index[b].display_name()))
                    });
                    ids.into_iter()
                        .filter(|id| !h.is_home_app_hidden(id))
                        .take(want)
                        .collect()
                } else {
                    if c.home_apps_mode == "recent" {
                        h.recent_apps(24)
                    } else {
                        h.top_apps(24)
                    }
                    .into_iter()
                    .filter(|id| !h.is_home_app_hidden(id) && app_index.contains_key(id))
                    .take(cap("Apps"))
                    .collect()
                };
                for id in app_ids {
                    let app = &app_index[&id];
                    let mut m = app_to_match(app, 1.0);
                    m.actions
                        .push(("hide_home", "Hide".to_string(), Action::HideHomeApp(id.clone())));
                    v.push(m);
                }
                let nfiles = if fs { HOME_ROW_FULL } else { cap("Files") };
                let file_paths: Vec<std::path::PathBuf> = if c.home_files_mode == "used" {
                    h.most_used_files(24)
                        .into_iter()
                        .map(std::path::PathBuf::from)
                        .collect()
                } else {
                    // "Recent" comes from the system recent-files list (the same
                    // one Nemo and the menu use), so it's populated immediately.
                    system_recent_files(24)
                }
                .into_iter()
                .filter(|p| {
                    let s = p.to_string_lossy();
                    p.exists() && !h.is_home_file_hidden(&s) && !h.is_file_hidden(&s)
                })
                .take(nfiles)
                .collect();
                for path in file_paths {
                    let mut m = file_to_match(&path, 1.0);
                    // The global "hide" doesn't belong on the home card — swap it
                    // for a home-only hide.
                    m.actions.retain(|(id, _, _)| *id != "hide");
                    m.actions.push((
                        "hide_home",
                        "Hide".to_string(),
                        Action::HideHomeFile(path.clone()),
                    ));
                    v.push(m);
                }
                v
            } else {
                registry.borrow().query(&query)
            };

            // Drop files the user has hidden, then apply action preferences.
            let mut results = results;
            {
                let h = history.borrow();
                results.retain(|m| match &m.action {
                    Action::OpenPath(p) => !h.is_file_hidden(&p.to_string_lossy()),
                    _ => true,
                });
            }
            for m in results.iter_mut() {
                apply_action_prefs(m, &cfg.borrow());
            }

            while let Some(child) = results_box.first_child() {
                results_box.remove(&child);
            }
            rows.borrow_mut().clear();

            // Dashboard (empty query) = a wrapping grid of cards per section.
            // Active search = vertical list of lettered rows with chips. On the
            // dashboard (but not full-screen) each section header carries an
            // expand/collapse toggle that switches between one and two rows.
            let dashboard = query.trim().is_empty();
            let fullscreen = cfg.borrow().fullscreen;
            let per_row = if fullscreen { HOME_ROW_FULL } else { HOME_ROW };
            let mut prev_cat = "";
            // Each dashboard section is a left-aligned vertical box sized to its
            // card rows, so the header (and its expand toggle) lines up with the
            // cards instead of stretching to the window edge. Cards wrap into
            // fixed-size rows: a new row starts every `per_row` cards.
            let mut cur_section: Option<GtkBox> = None;
            let mut cur_row: Option<GtkBox> = None;
            let mut cat_count = 0usize;
            for (i, m) in results.iter().enumerate() {
                let letter = char::from(b'A' + (i as u8).min(25));
                if m.category != prev_cat {
                    if dashboard {
                        let section = GtkBox::new(Orientation::Vertical, 0);
                        section.set_halign(Align::Start);
                        let expandable = !fullscreen;
                        let is_exp = expanded.borrow().get(m.category).copied().unwrap_or(false);
                        let (header, toggle) = home_section_header(m.category, expandable, is_exp);
                        if let Some(btn) = toggle {
                            let expanded = expanded.clone();
                            let refresh_slot = refresh_slot.clone();
                            let cat = m.category.to_string();
                            btn.connect_clicked(move |_| {
                                let cur = expanded.borrow().get(&cat).copied().unwrap_or(false);
                                expanded.borrow_mut().insert(cat.clone(), !cur);
                                if let Some(r) = refresh_slot.borrow().clone() {
                                    r();
                                }
                            });
                        }
                        section.append(&header);
                        results_box.append(&section);
                        cur_section = Some(section);
                    } else {
                        results_box.append(&section_header(m.category));
                    }
                    prev_cat = m.category;
                    cur_row = None;
                    cat_count = 0;
                }

                let gesture = gtk4::GestureClick::new();
                {
                    let run_primary = run_primary.clone();
                    gesture.connect_released(move |_, _, _, _| run_primary(i));
                }

                if dashboard {
                    // Start a fresh row every `per_row` cards in this section.
                    if cat_count % per_row == 0 {
                        let rb = GtkBox::new(Orientation::Horizontal, 8);
                        rb.set_halign(Align::Start);
                        rb.set_margin_start(8);
                        rb.set_margin_end(8);
                        rb.set_margin_top(4);
                        if let Some(s) = &cur_section {
                            s.append(&rb);
                        }
                        cur_row = Some(rb);
                    }
                    cat_count += 1;
                    // The full-screen app grid can hold hundreds of items, so
                    // per-card letters (which only address the first 26) are
                    // dropped there — navigate it by typing instead.
                    let badge = if fullscreen { None } else { Some(letter) };
                    let (card, chips) = build_card(badge, m);
                    for (n, chip) in chips.into_iter().enumerate() {
                        let run_secondary = run_secondary.clone();
                        chip.connect_clicked(move |_| run_secondary(i, n));
                    }
                    card.add_controller(gesture);
                    if let Some(rb) = &cur_row {
                        rb.append(&card);
                    }
                    rows.borrow_mut().push(card.upcast::<gtk4::Widget>());
                } else {
                    let (row, chips) = build_grouped_row(letter, m);
                    for (n, chip) in chips.into_iter().enumerate() {
                        let run_secondary = run_secondary.clone();
                        chip.connect_clicked(move |_| run_secondary(i, n));
                    }
                    row.add_controller(gesture);
                    results_box.append(&row);
                    rows.borrow_mut().push(row.upcast::<gtk4::Widget>());
                }
            }
            *matches.borrow_mut() = results;
            // Only paint a row as selected once the user is navigating the list.
            // While typing (focus in the search box) no row is highlighted — the
            // search bar's accent glow shows where focus is. The first Tab/↓ then
            // highlights row 0 correctly.
            if *nav_mode.borrow() && !rows.borrow().is_empty() {
                select(0);
            } else {
                *selected.borrow_mut() = 0;
            }
        })
    };
    *refresh_slot.borrow_mut() = Some(refresh.clone());

    // Typing → back to search mode + rebuild, then optionally auto-focus the
    // result list after the configured delay (so you can pick without Tab).
    entry.connect_changed({
        let refresh = refresh.clone();
        let nav_mode = nav_mode.clone();
        let cfg = cfg.clone();
        let select = select.clone();
        let focus_gen = focus_gen.clone();
        let set_search_glow = set_search_glow.clone();
        move |e| {
            *nav_mode.borrow_mut() = false;
            set_search_glow(true);
            refresh();
            let g = focus_gen.get().wrapping_add(1);
            focus_gen.set(g);
            let delay = cfg.borrow().focus_delay_ms;
            if delay > 0 && !e.text().trim().is_empty() {
                let nav_mode = nav_mode.clone();
                let select = select.clone();
                let focus_gen = focus_gen.clone();
                let set_search_glow = set_search_glow.clone();
                glib::timeout_add_local_once(
                    std::time::Duration::from_millis(delay as u64),
                    move || {
                        if focus_gen.get() == g && !*nav_mode.borrow() {
                            *nav_mode.borrow_mut() = true;
                            set_search_glow(false);
                            select(0);
                        }
                    },
                );
            }
        }
    });

    // Gear → settings window. Keep a handle so a second click focuses the
    // existing window instead of opening a duplicate.
    let settings_win: Rc<RefCell<Option<Window>>> = Rc::new(RefCell::new(None));
    gear.connect_clicked({
        let window = window.clone();
        let cfg = cfg.clone();
        let registry = registry.clone();
        let refresh = refresh.clone();
        let history = history.clone();
        let settings_win = settings_win.clone();
        move |_| {
            if let Some(w) = settings_win.borrow().clone() {
                w.present();
                return;
            }
            let w = open_settings(
                &window,
                cfg.clone(),
                registry.clone(),
                refresh.clone(),
                history.clone(),
            );
            // Forget the handle once it closes, so it can be reopened later.
            w.connect_close_request({
                let settings_win = settings_win.clone();
                move |_| {
                    *settings_win.borrow_mut() = None;
                    glib::Propagation::Proceed
                }
            });
            *settings_win.borrow_mut() = Some(w);
        }
    });

    // Full-screen toggle: switch the home into / out of the start-menu layout,
    // resize the window to match, and persist the choice.
    full_btn.connect_clicked({
        let cfg = cfg.clone();
        let window = window.clone();
        let scroller = scroller.clone();
        let refresh = refresh.clone();
        let full_btn = full_btn.clone();
        move |_| {
            let now = !cfg.borrow().fullscreen;
            cfg.borrow_mut().fullscreen = now;
            cfg.borrow().save();
            full_btn.set_icon_name(if now {
                "view-restore-symbolic"
            } else {
                "view-fullscreen-symbolic"
            });
            apply_window_mode(&window, &scroller, now);
            refresh();
            // Re-anchor the now differently-sized window. Two ticks: the first
            // lets the content resize, the second positions the final size.
            let position = cfg.borrow().position.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(90), move || {
                move_active_window(&position, now);
            });
        }
    });

    // Keyboard. Search mode: type normally; Enter runs the top result; Tab/↓
    // enter navigation. Nav mode: letters select rows, digits run their numbered
    // actions, Enter runs the selected row, Esc returns to typing.
    {
        let key = EventControllerKey::new();
        let nav_mode = nav_mode.clone();
        let selected = selected.clone();
        let rows = rows.clone();
        let select = select.clone();
        let run_primary = run_primary.clone();
        let run_secondary = run_secondary.clone();
        let win = window.clone();
        let entry = entry.clone();
        let set_search_glow = set_search_glow.clone();
        key.connect_key_pressed(move |_, keyval, _code, _state| {
            use glib::Propagation::{Proceed, Stop};
            let nav = *nav_mode.borrow();
            match keyval {
                gdk::Key::Escape => {
                    if nav {
                        *nav_mode.borrow_mut() = false;
                        set_search_glow(true);
                        entry.grab_focus();
                    } else {
                        win.set_visible(false);
                    }
                    Stop
                }
                gdk::Key::Tab | gdk::Key::Down => {
                    if !nav {
                        *nav_mode.borrow_mut() = true;
                        set_search_glow(false);
                        select(0);
                    } else {
                        let s = *selected.borrow();
                        select(s + 1);
                    }
                    Stop
                }
                gdk::Key::ISO_Left_Tab | gdk::Key::Up => {
                    if nav {
                        let s = *selected.borrow();
                        if s > 0 {
                            select(s - 1);
                        }
                    }
                    Stop
                }
                gdk::Key::Return | gdk::Key::KP_Enter => {
                    let s = *selected.borrow();
                    run_primary(s);
                    Stop
                }
                _ => {
                    if !nav {
                        return Proceed; // let the entry receive typed characters
                    }
                    if let Some(ch) = keyval.to_unicode() {
                        if ch.is_ascii_alphabetic() {
                            let idx = (ch.to_ascii_lowercase() as u8 - b'a') as usize;
                            if idx < rows.borrow().len() {
                                select(idx);
                            }
                            return Stop;
                        }
                        if ch.is_ascii_digit() && ch != '0' {
                            let n = (ch as u8 - b'1') as usize;
                            let s = *selected.borrow();
                            run_secondary(s, n);
                            return Stop;
                        }
                    }
                    Stop
                }
            }
        });
        key.set_propagation_phase(gtk4::PropagationPhase::Capture);
        window.add_controller(key);
    }

    // The toggle: show (cleared + focused + positioned) if hidden, else hide.
    let cfg_for_toggle = cfg.clone();
    let nav_for_toggle = nav_mode.clone();
    let scroller_for_toggle = scroller.clone();
    let expanded_for_toggle = expanded.clone();
    Rc::new(move || {
        if window.is_visible() {
            window.set_visible(false);
            return;
        }
        *nav_for_toggle.borrow_mut() = false;
        // Each open starts collapsed.
        expanded_for_toggle.borrow_mut().clear();
        set_search_glow(true);
        entry.set_text("");
        let fullscreen = cfg_for_toggle.borrow().fullscreen;
        apply_window_mode(&window, &scroller_for_toggle, fullscreen);
        refresh();
        window.present();
        entry.grab_focus();
        let position = cfg_for_toggle.borrow().position.clone();
        // Move once the window is mapped (GTK4 can't position on X11, so use the
        // window manager via xdotool).
        glib::timeout_add_local_once(std::time::Duration::from_millis(90), move || {
            move_active_window(&position, fullscreen);
        });
    })
}

/// Size the window for the current mode. We avoid the WM's (Cinnamon-flaky)
/// fullscreen state: full screen instead pins a fixed large minimum size so the
/// window stays big even while searching (a short result list won't shrink it);
/// normal mode clears that and content-fits up to a cap.
fn apply_window_mode(window: &ApplicationWindow, scroller: &ScrolledWindow, fullscreen: bool) {
    if fullscreen {
        let (_, _, mw, mh) = monitor_geometry();
        let fs_w = fullscreen_width().min(mw);
        let fs_h = (mh - 80).max(400);
        window.set_size_request(fs_w, fs_h);
        scroller.set_propagate_natural_height(false);
        scroller.set_max_content_height(-1);
        scroller.set_vexpand(true);
    } else {
        window.set_size_request(-1, -1);
        scroller.set_vexpand(false);
        scroller.set_propagate_natural_height(true);
        scroller.set_max_content_height(CONTENT_MAX_H);
    }
}

/// Estimated width of the full-screen start menu (rows of HOME_ROW_FULL cards).
fn fullscreen_width() -> i32 {
    HOME_ROW_FULL as i32 * (CARD_W + 8) + 32
}

/// Move the currently-active window to position using the window manager
/// (xdotool); GTK4 has no window-positioning API on X11. Full screen anchors the
/// large content-sized window near the top, horizontally centred.
fn move_active_window(position: &str, fullscreen: bool) {
    let (mx, my, mw, mh) = monitor_geometry();
    let (x, y) = if fullscreen {
        let ww = fullscreen_width().min(mw);
        (mx + (mw - ww) / 2, my + 40)
    } else {
        let (ww, wh) = (WIN_W, WIN_H);
        let y = my
            + match position {
                "top" => mh / 10,
                "bottom" => mh * 7 / 10,
                _ => (mh - wh) / 2, // center
            };
        (mx + (mw - ww) / 2, y)
    };
    let _ = std::process::Command::new("xdotool")
        .args([
            "getactivewindow",
            "windowmove",
            &x.to_string(),
            &y.to_string(),
        ])
        .spawn();
}


/// Turn a stored clipboard entry into a result row, with Pin/Hide actions.
fn clip_to_match(c: ClipEntry) -> Match {
    let pin_label = if c.pinned { "Unpin" } else { "Pin" };
    let secondary = vec![
        ("pin", pin_label.to_string(), Action::PinClip(c.id.clone())),
        ("remove", "Remove".to_string(), Action::RemoveClip(c.id.clone())),
    ];
    let (title, icon, primary) = if c.kind == "image" {
        (
            "Image".to_string(),
            Some("image-x-generic".to_string()),
            Action::CopyImage(c.path),
        )
    } else {
        let mut t = c.text.replace('\n', " ");
        if t.chars().count() > 80 {
            t = t.chars().take(80).collect::<String>() + "…";
        }
        (t, Some("edit-paste".to_string()), Action::Copy(c.text))
    };
    Match {
        title,
        subtitle: if c.pinned { "📌 Clipboard" } else { "Clipboard" }.to_string(),
        icon,
        score: 1.0,
        category: "Clipboard",
        action: primary,
        actions: secondary,
        tag: None,
    }
}

fn data_home() -> Option<std::path::PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share")))
}

/// Where the section icons are written for runtime use.
fn asset_dir() -> Option<std::path::PathBuf> {
    Some(data_home()?.join("grun/assets"))
}

/// Standalone copy of the app icon, referenced by absolute path in the desktop
/// entry (so it shows even when the user icon theme has no cache/index).
fn app_icon_file() -> Option<std::path::PathBuf> {
    Some(data_home()?.join("grun/icon.png"))
}

/// Write the embedded section icons to disk on first run.
fn ensure_assets() {
    let Some(dir) = asset_dir() else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    for (name, bytes) in [
        ("Dark-Clipboard.svg", EMB_CLIPBOARD),
        ("Dark-apps.svg", EMB_APPS),
        ("Dark-Files.svg", EMB_FILES),
        ("Dark-search.svg", EMB_SEARCH),
        ("Dark-Ai.svg", EMB_AI),
        ("Light-Clipboard.svg", EMB_CLIPBOARD_LIGHT),
        ("Light-apps.svg", EMB_APPS_LIGHT),
        ("Light-Files.svg", EMB_FILES_LIGHT),
        ("Light-search.svg", EMB_SEARCH_LIGHT),
        ("Light-AI.svg", EMB_AI_LIGHT),
        ("Dark-pointer-Down.svg", EMB_DOWN),
        ("Dark-pointer-up.svg", EMB_UP),
        ("Light-pointer-Down.svg", EMB_DOWN_LIGHT),
        ("Light-pointer-up.svg", EMB_UP_LIGHT),
    ] {
        let p = dir.join(name);
        if !p.exists() {
            let _ = std::fs::write(&p, bytes);
        }
    }
}

/// An expand (down) or collapse (up) arrow image, themed light/dark.
fn pointer_image(expanded: bool) -> Option<Image> {
    let file = match (expanded, prefer_dark()) {
        (false, true) => "Dark-pointer-Down.svg",
        (false, false) => "Light-pointer-Down.svg",
        (true, true) => "Dark-pointer-up.svg",
        (true, false) => "Light-pointer-up.svg",
    };
    let path = asset_dir()?.join(file);
    path.exists().then(|| Image::from_file(path))
}

/// Whether the active GTK theme is a dark one. Used to pick light vs dark
/// section icons (the CSS colours follow the theme on their own).
pub(crate) fn prefer_dark() -> bool {
    if let Some(s) = gtk4::Settings::default() {
        if s.property::<bool>("gtk-application-prefer-dark-theme") {
            return true;
        }
        return s
            .property::<String>("gtk-theme-name")
            .to_lowercase()
            .contains("dark");
    }
    true
}

/// Path to the theme-appropriate AI icon, used as the AI result's row icon.
pub(crate) fn ai_row_icon() -> Option<String> {
    let file = if prefer_dark() {
        "Dark-Ai.svg"
    } else {
        "Light-AI.svg"
    };
    let p = asset_dir()?.join(file);
    p.exists().then(|| p.to_string_lossy().to_string())
}

/// Install a desktop entry so the window manager can match grun's window to its
/// icon (Cinnamon matches the window's app-id to `<app-id>.desktop`). Without
/// this, a launcher run from a bare binary shows a generic icon.
fn install_desktop_entry() {
    let Some(base) = data_home() else {
        return;
    };
    let dir = base.join("applications");
    let _ = std::fs::create_dir_all(&dir);
    let exe = std::env::current_exe()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_else(|_| "grun".to_string());
    // Absolute path to the icon file, so it resolves without an icon-theme cache.
    let icon = app_icon_file()
        .filter(|p| p.exists())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| APP_ID.to_string());
    let body = format!(
        "[Desktop Entry]\nType=Application\nName=grun\nGenericName=Application Launcher\nComment=Search apps, files, clipboard and more\nExec={exe}\nIcon={icon}\nTerminal=false\nCategories=Utility;\nKeywords=launcher;search;run;\nStartupWMClass=grun\n"
    );
    let _ = std::fs::write(dir.join(format!("{APP_ID}.desktop")), body);
}

/// Install grun's app icon (the embedded PNG) into the user icon theme. GTK4
/// automatically uses a themed icon named after the application id.
fn install_app_icon() {
    let Some(base) = data_home() else {
        return;
    };
    // Drop the older scalable search icon if it lingers.
    let _ = std::fs::remove_file(base.join(format!("icons/hicolor/scalable/apps/{APP_ID}.svg")));
    for (size, bytes) in [(256u32, EMB_APPICON_256), (512, EMB_APPICON_512)] {
        let dir = base.join(format!("icons/hicolor/{size}x{size}/apps"));
        let target = dir.join(format!("{APP_ID}.png"));
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(&target, bytes);
    }
    // Standalone copy referenced by absolute path from the desktop entry.
    if let Some(p) = app_icon_file() {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&p, EMB_APPICON_256);
    }
}

/// Path to grun's autostart desktop entry.
fn autostart_path() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))?;
    Some(base.join("autostart").join("grun.desktop"))
}

fn autostart_enabled() -> bool {
    autostart_path().map(|p| p.exists()).unwrap_or(false)
}

/// Enable/disable launching grun at login by writing/removing an autostart entry.
fn set_autostart(on: bool) {
    let Some(path) = autostart_path() else {
        return;
    };
    if on {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let exe = std::env::current_exe()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_else(|_| "grun".to_string());
        let body = format!(
            "[Desktop Entry]\nType=Application\nName=grun\nComment=Application launcher\nExec={exe}\nIcon={APP_ID}\nX-GNOME-Autostart-enabled=true\nNoDisplay=true\n"
        );
        let _ = std::fs::write(&path, body);
    } else {
        let _ = std::fs::remove_file(&path);
    }
}

/// Most recently used local files, from the freedesktop recent-files registry
/// (`recently-used.xbel`) — the same list Nemo and the Cinnamon menu show.
fn system_recent_files(n: usize) -> Vec<std::path::PathBuf> {
    let mgr = gtk4::RecentManager::default();
    let mut items: Vec<_> = mgr
        .items()
        .into_iter()
        .filter(|i| i.is_local() && i.exists())
        .collect();
    items.sort_by_key(|i| std::cmp::Reverse(i.visited().to_unix()));
    items
        .into_iter()
        .filter_map(|i| gtk4::gio::File::for_uri(i.uri().as_str()).path())
        .take(n)
        .collect()
}

/// Geometry (x, y, width, height) of the primary monitor.
fn monitor_geometry() -> (i32, i32, i32, i32) {
    if let Some(display) = gdk::Display::default() {
        if let Some(obj) = display.monitors().item(0) {
            if let Ok(m) = obj.downcast::<gdk::Monitor>() {
                let g = m.geometry();
                return (g.x(), g.y(), g.width(), g.height());
            }
        }
    }
    (0, 0, 1920, 1080)
}

/// Open the settings window: the function list, in priority order, each with
/// up/down reorder buttons and an on/off switch. Changes apply live and save.
fn open_settings(
    parent: &ApplicationWindow,
    cfg: Rc<RefCell<Config>>,
    registry: Rc<RefCell<Registry>>,
    refresh: Rc<dyn Fn()>,
    history: Rc<RefCell<History>>,
) -> Window {
    let outer = GtkBox::new(Orientation::Vertical, 6);
    outer.set_margin_top(18);
    outer.set_margin_bottom(18);
    outer.set_margin_start(18);
    outer.set_margin_end(18);

    let title = Label::new(Some("Functions — drag priority with ↑ ↓, toggle on/off"));
    title.set_halign(Align::Start);
    title.add_css_class("grun-settings-title");
    outer.append(&title);

    // The list of function rows, rebuilt whenever the order changes.
    let list = GtkBox::new(Orientation::Vertical, 4);
    outer.append(&list);

    // Apply a config change everywhere: persist, rebuild registry, re-run query.
    // Rebuilding the registry re-enumerates every installed app, so we defer the
    // work to the next idle tick — that lets a toggle switch finish its animation
    // immediately instead of stuttering while the rebuild runs.
    let apply: Rc<dyn Fn()> = {
        let cfg = cfg.clone();
        let registry = registry.clone();
        let refresh = refresh.clone();
        let history = history.clone();
        Rc::new(move || {
            let cfg = cfg.clone();
            let registry = registry.clone();
            let refresh = refresh.clone();
            let history = history.clone();
            glib::idle_add_local_once(move || {
                cfg.borrow().save();
                *registry.borrow_mut() = Registry::from_config(&cfg.borrow(), &history);
                refresh();
            });
        })
    };

    // Rebuild the rows from the current config order. The reorder buttons need
    // to trigger another rebuild, so we late-bind the closure into a slot.
    let slot: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let rebuild: Rc<dyn Fn()> = {
        let slot = slot.clone();
        let list = list.clone();
        let cfg = cfg.clone();
        let apply = apply.clone();
        Rc::new(move || {
            while let Some(child) = list.first_child() {
                list.remove(&child);
            }
            let count = cfg.borrow().providers.len();
            for i in 0..count {
                let (id, enabled) = {
                    let c = cfg.borrow();
                    (c.providers[i].id.clone(), c.providers[i].enabled)
                };

                let up = Button::from_icon_name("go-up-symbolic");
                up.set_sensitive(i > 0);
                up.connect_clicked({
                    let slot = slot.clone();
                    let cfg = cfg.clone();
                    let apply = apply.clone();
                    move |_| {
                        cfg.borrow_mut().move_item(i, -1);
                        apply();
                        if let Some(rb) = slot.borrow().clone() {
                            rb();
                        }
                    }
                });

                let down = Button::from_icon_name("go-down-symbolic");
                down.set_sensitive(i + 1 < count);
                down.connect_clicked({
                    let slot = slot.clone();
                    let cfg = cfg.clone();
                    let apply = apply.clone();
                    move |_| {
                        cfg.borrow_mut().move_item(i, 1);
                        apply();
                        if let Some(rb) = slot.borrow().clone() {
                            rb();
                        }
                    }
                });

                let label = Label::new(Some(config::label(&id)));
                label.set_halign(Align::Start);
                label.set_hexpand(true);

                let sw = Switch::new();
                sw.set_active(enabled);
                sw.set_valign(Align::Center);
                sw.connect_state_set({
                    let cfg = cfg.clone();
                    let apply = apply.clone();
                    move |_, state| {
                        cfg.borrow_mut().set_enabled(i, state);
                        apply();
                        glib::Propagation::Proceed
                    }
                });

                let row = GtkBox::new(Orientation::Horizontal, 8);
                row.append(&up);
                row.append(&down);
                row.append(&label);
                row.append(&sw);
                list.append(&row);
            }
        })
    };
    *slot.borrow_mut() = Some(rebuild.clone());
    rebuild();

    // Window position selector.
    let pos_title = Label::new(Some("Pop up at"));
    pos_title.set_halign(Align::Start);
    pos_title.set_margin_top(12);
    pos_title.add_css_class("grun-settings-title");
    outer.append(&pos_title);

    let cur_pos = cfg.borrow().position.clone();
    add_choice_row(
        &outer,
        "",
        &[("Top", "top"), ("Center", "center"), ("Bottom", "bottom")],
        &cur_pos,
        {
            let cfg = cfg.clone();
            Rc::new(move |v: &str| {
                cfg.borrow_mut().set_position(v);
                cfg.borrow().save();
            })
        },
    );

    // --- Home dashboard options ---
    let home_title = Label::new(Some("Home dashboard"));
    home_title.add_css_class("grun-settings-title");
    home_title.set_halign(Align::Start);
    home_title.set_margin_top(12);
    outer.append(&home_title);

    let cur_apps = cfg.borrow().home_apps_mode.clone();
    add_choice_row(
        &outer,
        "Apps",
        &[("Most used", "used"), ("Recent", "recent")],
        &cur_apps,
        {
            let cfg = cfg.clone();
            let refresh = refresh.clone();
            Rc::new(move |v: &str| {
                cfg.borrow_mut().home_apps_mode = v.to_string();
                cfg.borrow().save();
                refresh();
            })
        },
    );
    let cur_files = cfg.borrow().home_files_mode.clone();
    add_choice_row(
        &outer,
        "Files",
        &[("Recent", "recent"), ("Most used", "used")],
        &cur_files,
        {
            let cfg = cfg.clone();
            let refresh = refresh.clone();
            Rc::new(move |v: &str| {
                cfg.borrow_mut().home_files_mode = v.to_string();
                cfg.borrow().save();
                refresh();
            })
        },
    );
    let cur_delay = cfg.borrow().focus_delay_ms.to_string();
    add_choice_row(
        &outer,
        "Auto-focus list",
        &[("Off", "0"), ("0.2s", "200"), ("0.5s", "500"), ("1s", "1000")],
        &cur_delay,
        {
            let cfg = cfg.clone();
            Rc::new(move |v: &str| {
                cfg.borrow_mut().focus_delay_ms = v.parse().unwrap_or(0);
                cfg.borrow().save();
            })
        },
    );

    // --- Toggles, grouped together ---
    let toggles_title = Label::new(Some("Options"));
    toggles_title.add_css_class("grun-settings-title");
    toggles_title.set_halign(Align::Start);
    toggles_title.set_margin_top(12);
    outer.append(&toggles_title);

    // Show the clipboard section on the home dashboard.
    add_switch_row(&outer, "Show clipboard", cfg.borrow().home_clipboard, {
        let cfg = cfg.clone();
        let refresh = refresh.clone();
        Rc::new(move |state| {
            cfg.borrow_mut().home_clipboard = state;
            let cfg = cfg.clone();
            let refresh = refresh.clone();
            glib::idle_add_local_once(move || {
                cfg.borrow().save();
                refresh();
            });
        })
    });

    // Match app descriptions/keywords too (e.g. "screenshot" → Flameshot).
    add_switch_row(&outer, "Search app descriptions", cfg.borrow().search_descriptions, {
        let cfg = cfg.clone();
        let registry = registry.clone();
        let refresh = refresh.clone();
        let history = history.clone();
        Rc::new(move |state| {
            cfg.borrow_mut().search_descriptions = state;
            let cfg = cfg.clone();
            let registry = registry.clone();
            let refresh = refresh.clone();
            let history = history.clone();
            glib::idle_add_local_once(move || {
                cfg.borrow().save();
                *registry.borrow_mut() = Registry::from_config(&cfg.borrow(), &history);
                refresh();
            });
        })
    });

    // Start on login (writes/removes an autostart .desktop).
    add_switch_row(&outer, "Start on login", autostart_enabled(), {
        Rc::new(move |state| {
            glib::idle_add_local_once(move || set_autostart(state));
        })
    });

    // --- Actions per category (reorder / enable) ---
    let act_title = Label::new(Some("Actions per category"));
    act_title.add_css_class("grun-settings-title");
    act_title.set_halign(Align::Start);
    act_title.set_margin_top(12);
    outer.append(&act_title);
    for &(category, _) in config::KNOWN_ACTIONS {
        add_reorderable_actions(&outer, category, cfg.clone(), refresh.clone());
    }

    add_home_hidden(&outer, history.clone(), refresh.clone());
    add_hidden_files(&outer, history.clone(), refresh.clone());

    let save_btn = Button::with_label("Save & Close");
    save_btn.add_css_class("grun-save");
    save_btn.set_margin_top(16);
    outer.append(&save_btn);

    let scroller = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vexpand(true)
        .child(&outer)
        .build();
    let win = Window::builder()
        .title("grun settings")
        .transient_for(parent)
        .default_width(380)
        .default_height(620)
        .modal(false)
        .child(&scroller)
        .build();

    save_btn.connect_clicked({
        let cfg = cfg.clone();
        let win = win.clone();
        move |_| {
            cfg.borrow().save();
            win.close();
        }
    });
    win.present();
    win
}

/// A list of hidden files, each with a Restore button to un-hide it.
/// Apps and files the user removed from the home dashboard, each with a Restore
/// button. These are home-only hides (the items still show up in search).
fn add_home_hidden(outer: &GtkBox, history: Rc<RefCell<History>>, refresh: Rc<dyn Fn()>) {
    let header = Label::new(Some("Hidden from home"));
    header.add_css_class("grun-settings-title");
    header.set_halign(Align::Start);
    header.set_margin_top(12);
    outer.append(&header);

    let list = GtkBox::new(Orientation::Vertical, 4);
    outer.append(&list);

    let slot: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let rebuild: Rc<dyn Fn()> = {
        let slot = slot.clone();
        let list = list.clone();
        let history = history.clone();
        let refresh = refresh.clone();
        Rc::new(move || {
            while let Some(c) = list.first_child() {
                list.remove(&c);
            }
            let apps = history.borrow().home_hidden_apps();
            let files = history.borrow().home_hidden_files();
            if apps.is_empty() && files.is_empty() {
                let none = Label::new(Some("None"));
                none.add_css_class("grun-sub");
                none.set_halign(Align::Start);
                list.append(&none);
                return;
            }
            // Apps: resolve the .desktop id to a friendly name where we can.
            for id in apps {
                let name = gtk4::gio::DesktopAppInfo::new(&id)
                    .map(|a| a.display_name().to_string())
                    .unwrap_or_else(|| id.trim_end_matches(".desktop").to_string());
                let row = home_hidden_row(&format!("{name}  ·  App"), &id);
                let restore = Button::with_label("Restore");
                restore.connect_clicked({
                    let slot = slot.clone();
                    let history = history.clone();
                    let refresh = refresh.clone();
                    let id = id.clone();
                    move |_| {
                        history.borrow_mut().unhide_home_app(&id);
                        history.borrow().save();
                        refresh();
                        if let Some(rb) = slot.borrow().clone() {
                            rb();
                        }
                    }
                });
                row.append(&restore);
                list.append(&row);
            }
            // Files: show the basename, tooltip the full path.
            for path in files {
                let name = std::path::Path::new(&path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.clone());
                let row = home_hidden_row(&format!("{name}  ·  File"), &path);
                let restore = Button::with_label("Restore");
                restore.connect_clicked({
                    let slot = slot.clone();
                    let history = history.clone();
                    let refresh = refresh.clone();
                    let path = path.clone();
                    move |_| {
                        history.borrow_mut().unhide_home_file(&path);
                        history.borrow().save();
                        refresh();
                        if let Some(rb) = slot.borrow().clone() {
                            rb();
                        }
                    }
                });
                row.append(&restore);
                list.append(&row);
            }
        })
    };
    *slot.borrow_mut() = Some(rebuild.clone());
    rebuild();
}

/// A row for the "Hidden from home" list: an ellipsized label (tooltip = `tip`)
/// that expands, ready for a trailing Restore button.
fn home_hidden_row(text: &str, tip: &str) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 8);
    let lbl = Label::new(Some(text));
    lbl.set_halign(Align::Start);
    lbl.set_hexpand(true);
    lbl.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
    lbl.set_tooltip_text(Some(tip));
    row.append(&lbl);
    row
}

fn add_hidden_files(outer: &GtkBox, history: Rc<RefCell<History>>, refresh: Rc<dyn Fn()>) {
    let header = Label::new(Some("Hidden files"));
    header.add_css_class("grun-settings-title");
    header.set_halign(Align::Start);
    header.set_margin_top(12);
    outer.append(&header);

    let list = GtkBox::new(Orientation::Vertical, 4);
    outer.append(&list);

    let slot: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let rebuild: Rc<dyn Fn()> = {
        let slot = slot.clone();
        let list = list.clone();
        let history = history.clone();
        let refresh = refresh.clone();
        Rc::new(move || {
            while let Some(c) = list.first_child() {
                list.remove(&c);
            }
            let hidden = history.borrow().hidden_files();
            if hidden.is_empty() {
                let none = Label::new(Some("None"));
                none.add_css_class("grun-sub");
                none.set_halign(Align::Start);
                list.append(&none);
                return;
            }
            for path in hidden {
                let row = GtkBox::new(Orientation::Horizontal, 8);
                let name = std::path::Path::new(&path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.clone());
                let lbl = Label::new(Some(&name));
                lbl.set_halign(Align::Start);
                lbl.set_hexpand(true);
                lbl.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
                lbl.set_tooltip_text(Some(&path));
                let restore = Button::with_label("Restore");
                restore.connect_clicked({
                    let slot = slot.clone();
                    let history = history.clone();
                    let refresh = refresh.clone();
                    let path = path.clone();
                    move |_| {
                        history.borrow_mut().unhide_file(&path);
                        history.borrow().save();
                        refresh();
                        if let Some(rb) = slot.borrow().clone() {
                            rb();
                        }
                    }
                });
                row.append(&lbl);
                row.append(&restore);
                list.append(&row);
            }
        })
    };
    *slot.borrow_mut() = Some(rebuild.clone());
    rebuild();
}

/// A reorderable, toggleable list of a category's secondary actions.
fn add_reorderable_actions(
    outer: &GtkBox,
    category: &'static str,
    cfg: Rc<RefCell<Config>>,
    refresh: Rc<dyn Fn()>,
) {
    let header = Label::new(Some(category));
    header.add_css_class("grun-section");
    header.set_halign(Align::Start);
    header.set_margin_top(8);
    outer.append(&header);

    // For Search/AI the topmost enabled entry is the default (Enter) action.
    if config::primary_selectable(category) {
        let hint = Label::new(Some("Top enabled = default (Enter); the rest are side actions"));
        hint.add_css_class("grun-sub");
        hint.set_halign(Align::Start);
        hint.set_wrap(true);
        outer.append(&hint);
    }

    let list = GtkBox::new(Orientation::Vertical, 4);
    outer.append(&list);

    let slot: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let rebuild: Rc<dyn Fn()> = {
        let slot = slot.clone();
        let list = list.clone();
        let cfg = cfg.clone();
        let refresh = refresh.clone();
        Rc::new(move || {
            while let Some(c) = list.first_child() {
                list.remove(&c);
            }
            let order = cfg.borrow().action_order(category);
            let count = order.len();
            for (i, (id, enabled)) in order.into_iter().enumerate() {
                let up = Button::from_icon_name("go-up-symbolic");
                up.set_sensitive(i > 0);
                up.connect_clicked({
                    let slot = slot.clone();
                    let cfg = cfg.clone();
                    let refresh = refresh.clone();
                    move |_| {
                        cfg.borrow_mut().move_action(category, i, -1);
                        cfg.borrow().save();
                        refresh();
                        if let Some(rb) = slot.borrow().clone() {
                            rb();
                        }
                    }
                });
                let down = Button::from_icon_name("go-down-symbolic");
                down.set_sensitive(i + 1 < count);
                down.connect_clicked({
                    let slot = slot.clone();
                    let cfg = cfg.clone();
                    let refresh = refresh.clone();
                    move |_| {
                        cfg.borrow_mut().move_action(category, i, 1);
                        cfg.borrow().save();
                        refresh();
                        if let Some(rb) = slot.borrow().clone() {
                            rb();
                        }
                    }
                });
                let lbl = Label::new(Some(config::action_label(&id)));
                lbl.set_halign(Align::Start);
                lbl.set_hexpand(true);
                let sw = Switch::new();
                sw.set_active(enabled);
                sw.set_valign(Align::Center);
                sw.connect_state_set({
                    let cfg = cfg.clone();
                    let refresh = refresh.clone();
                    move |_, state| {
                        cfg.borrow_mut().set_action_enabled(category, i, state);
                        let cfg = cfg.clone();
                        let refresh = refresh.clone();
                        glib::idle_add_local_once(move || {
                            cfg.borrow().save();
                            refresh();
                        });
                        glib::Propagation::Proceed
                    }
                });
                let row = GtkBox::new(Orientation::Horizontal, 8);
                row.append(&up);
                row.append(&down);
                row.append(&lbl);
                row.append(&sw);
                list.append(&row);
            }
        })
    };
    *slot.borrow_mut() = Some(rebuild.clone());
    rebuild();
}

/// Execute an action and update history. Returns true if the window should
/// close afterward (pin/hide stay open so you can keep curating).
fn perform(action: &Action, history: &Rc<RefCell<History>>) -> bool {
    match action {
        Action::PinClip(id) => {
            let pinned = history
                .borrow()
                .clips
                .iter()
                .find(|c| &c.id == id)
                .map(|c| c.pinned)
                .unwrap_or(false);
            history.borrow_mut().set_pinned(id, !pinned);
            history.borrow().save();
            false
        }
        Action::RemoveClip(id) => {
            history.borrow_mut().remove_clip(id);
            history.borrow().save();
            false
        }
        Action::HideFile(path) => {
            history.borrow_mut().hide_file(&path.to_string_lossy());
            history.borrow().save();
            false
        }
        Action::HideHomeApp(id) => {
            history.borrow_mut().hide_home_app(id);
            history.borrow().save();
            false
        }
        Action::HideHomeFile(path) => {
            history.borrow_mut().hide_home_file(&path.to_string_lossy());
            history.borrow().save();
            false
        }
        Action::LaunchApp(info) => {
            if let Some(id) = info.id() {
                history.borrow_mut().record_app_launch(id.as_str());
                history.borrow().save();
            }
            action.run();
            true
        }
        Action::OpenPath(p) => {
            history.borrow_mut().record_file(&p.to_string_lossy());
            history.borrow().save();
            action.run();
            true
        }
        _ => {
            action.run();
            true
        }
    }
}

/// Reorder and filter a match's secondary actions per the category's config.
/// For *primary-selectable* categories (Search, AI) the first enabled action
/// becomes the match's default (Enter) action and the rest stay as side actions,
/// so reordering in settings changes which engine/assistant Enter uses.
fn apply_action_prefs(m: &mut Match, cfg: &Config) {
    let order = cfg.action_order(m.category);
    if order.is_empty() {
        return;
    }
    let mut remaining = std::mem::take(&mut m.actions);
    let mut new = Vec::new();
    for (id, enabled) in &order {
        if let Some(pos) = remaining.iter().position(|(aid, _, _)| aid == id) {
            let item = remaining.remove(pos);
            if *enabled {
                new.push(item);
            }
        }
    }
    new.extend(remaining); // unknown ids stay

    if config::primary_selectable(m.category) && !new.is_empty() {
        let (_, label, action) = new.remove(0);
        m.subtitle = format!("Press Enter to use {label}");
        m.action = action;
    }
    m.actions = new;
}

/// A settings row with a label and an on/off switch. `on_set` runs whenever the
/// switch flips. Used to keep all the toggles styled and grouped consistently.
fn add_switch_row(parent: &GtkBox, label: &str, active: bool, on_set: Rc<dyn Fn(bool)>) {
    let row = GtkBox::new(Orientation::Horizontal, 12);
    let lbl = Label::new(Some(label));
    lbl.set_halign(Align::Start);
    lbl.set_hexpand(true);
    let sw = Switch::new();
    sw.set_active(active);
    sw.set_valign(Align::Center);
    sw.connect_state_set(move |_, state| {
        on_set(state);
        glib::Propagation::Proceed
    });
    row.append(&lbl);
    row.append(&sw);
    parent.append(&row);
}

/// A settings row: a label and a set of choice buttons that call `pick(value)`.
/// The button matching `current` is highlighted, and the highlight follows
/// clicks.
fn add_choice_row(
    parent: &GtkBox,
    title: &str,
    opts: &[(&str, &str)],
    current: &str,
    pick: Rc<dyn Fn(&str)>,
) {
    let row = GtkBox::new(Orientation::Horizontal, 8);
    let lbl = Label::new(Some(title));
    lbl.set_halign(Align::Start);
    lbl.set_hexpand(true);
    row.append(&lbl);

    let buttons: Rc<RefCell<Vec<Button>>> = Rc::new(RefCell::new(Vec::new()));
    for (text, val) in opts {
        let btn = Button::with_label(text);
        btn.add_css_class("grun-choice");
        if *val == current {
            btn.add_css_class("active");
        }
        btn.connect_clicked({
            let pick = pick.clone();
            let val = val.to_string();
            let buttons = buttons.clone();
            let me = btn.clone();
            move |_| {
                pick(&val);
                for b in buttons.borrow().iter() {
                    b.remove_css_class("active");
                }
                me.add_css_class("active");
            }
        });
        buttons.borrow_mut().push(btn.clone());
        row.append(&btn);
    }
    parent.append(&row);
}

/// Section header: category SVG icon (from Assets) + label.
fn section_header(category: &str) -> GtkBox {
    let h = GtkBox::new(Orientation::Horizontal, 8);
    h.set_halign(Align::Start);
    h.set_margin_start(8);
    h.set_margin_top(10);
    if let Some(img) = section_icon(category) {
        img.set_pixel_size(18);
        h.append(&img);
    }
    let lbl = Label::new(Some(category));
    lbl.add_css_class("grun-section");
    h.append(&lbl);
    h
}

/// CSS class colouring a package-type tag (deb/system red, AppImage blue,
/// Flatpak green, Snap orange).
fn tag_class(tag: &str) -> &'static str {
    match tag {
        "AppImage" => "grun-tag-appimage",
        "Flatpak" => "grun-tag-flatpak",
        "Snap" => "grun-tag-snap",
        _ => "grun-tag-system", // "System" (deb/native) and anything else
    }
}

/// Home dashboard section header: the section header plus, when `expandable`, a
/// right-aligned expand/collapse toggle. Returns the header and the toggle (to
/// wire) if one was added.
fn home_section_header(category: &str, expandable: bool, expanded: bool) -> (GtkBox, Option<Button>) {
    let bar = GtkBox::new(Orientation::Horizontal, 8);
    bar.set_hexpand(true);
    let inner = section_header(category);
    inner.set_hexpand(true);
    bar.append(&inner);
    if expandable {
        // Label + arrow icon together.
        let content = GtkBox::new(Orientation::Horizontal, 6);
        let lbl = Label::new(Some(if expanded { "collapse" } else { "expand" }));
        content.append(&lbl);
        if let Some(img) = pointer_image(expanded) {
            img.set_pixel_size(16);
            content.append(&img);
        }
        let btn = Button::new();
        btn.set_child(Some(&content));
        btn.set_tooltip_text(Some(if expanded { "Collapse" } else { "Show more" }));
        btn.add_css_class("grun-chip");
        btn.set_valign(Align::Center);
        btn.set_halign(Align::End);
        btn.set_margin_end(8);
        bar.append(&btn);
        (bar, Some(btn))
    } else {
        (bar, None)
    }
}

/// Load a category's icon from the Assets folder, if present, picking the dark
/// or light variant to suit the active system theme.
fn section_icon(category: &str) -> Option<Image> {
    let dark = prefer_dark();
    let file = match (category, dark) {
        ("Clipboard", true) => "Dark-Clipboard.svg",
        ("Clipboard", false) => "Light-Clipboard.svg",
        ("Apps", true) => "Dark-apps.svg",
        ("Apps", false) => "Light-apps.svg",
        ("Files", true) => "Dark-Files.svg",
        ("Files", false) => "Light-Files.svg",
        ("Search", true) => "Dark-search.svg",
        ("Search", false) => "Light-search.svg",
        ("AI", true) => "Dark-Ai.svg",
        ("AI", false) => "Light-AI.svg",
        _ => return None,
    };
    let path = asset_dir()?.join(file);
    if path.exists() {
        Some(Image::from_file(path))
    } else {
        None
    }
}

/// Build a dashboard card matching the container mockups: a left column with
/// the letter + side-action buttons, and a roomy main area (icon/image + title
/// + path/tag). Returns the card and its side-action buttons for wiring.
fn build_card(letter: Option<char>, m: &Match) -> (GtkBox, Vec<Button>) {
    // Left column: letter badge over stacked side-action buttons.
    let left = GtkBox::new(Orientation::Vertical, 6);
    left.set_valign(Align::Start);
    left.set_size_request(96, -1);

    // The letter sits in the first box of the column (same shape as the side
    // actions), with the side-action buttons stacked below it. Full-screen cards
    // omit it.
    if let Some(letter) = letter {
        let badge = Label::new(Some(&letter.to_string()));
        badge.add_css_class("grun-card-letter");
        badge.set_hexpand(true);
        left.append(&badge);
    }

    let mut chips = Vec::new();
    for (_id, label, _) in m.actions.iter() {
        // Bound the button label so a long action name can't widen the card.
        let lbl = Label::new(Some(label));
        lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        lbl.set_max_width_chars(10);
        let b = Button::builder().child(&lbl).build();
        b.add_css_class("grun-side");
        b.set_hexpand(true);
        left.append(&b);
        chips.push(b);
    }

    // Main content area.
    let main = GtkBox::new(Orientation::Vertical, 6);
    main.set_hexpand(true);
    main.set_valign(Align::Center);

    let is_image_clip = matches!(&m.action, Action::CopyImage(_));
    // A text clip that's really an image-file path → show its thumbnail.
    let clip_img = if let Action::Copy(full) = &m.action {
        clipboard_image_path(full)
    } else {
        None
    };

    if m.category == "Clipboard" && !is_image_clip && clip_img.is_none() {
        // Text clip: wrapped over several lines, but bounded so the card keeps
        // its fixed width instead of stretching to fit a long line.
        let t = Label::new(Some(&m.title));
        t.set_wrap(true);
        t.set_lines(3);
        t.set_max_width_chars(16);
        t.set_width_chars(16);
        t.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        t.set_halign(Align::Start);
        t.set_xalign(0.0);
        t.add_css_class("grun-card-title");
        main.append(&t);
    } else {
        // Pick a thumbnail source: a copied image, an image-path clip, or an
        // image file in the Files section; otherwise the MIME icon.
        let thumb = match &m.action {
            Action::CopyImage(p) => Some(p.clone()),
            _ if clip_img.is_some() => clip_img.clone(),
            _ if m.category == "Files" && is_image_path(&m.subtitle) => Some(m.subtitle.clone()),
            _ => None,
        };
        if let Some(p) = thumb {
            let img = thumbnail(&p);
            img.set_halign(Align::Center);
            main.append(&img);
        } else {
            let icon = icon_image(m.icon.as_deref(), 52);
            icon.set_halign(Align::Center);
            main.append(&icon);
        }
        if let Some(tag) = &m.tag {
            let badge = Label::new(Some(tag));
            badge.add_css_class("grun-tag");
            badge.add_css_class(tag_class(tag));
            // Centred on its own line directly under the icon.
            badge.set_halign(Align::Center);
            main.append(&badge);
        }
        // Skip the redundant "Image" title for image clips.
        if !(is_image_clip || clip_img.is_some()) {
            let title = Label::new(Some(&m.title));
            title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            // Bound the natural width so a long name can't widen the card.
            title.set_max_width_chars(16);
            title.set_halign(Align::Center);
            title.add_css_class("grun-card-title");
            main.append(&title);
        }
    }
    // Path line for files (and image clips).
    if !m.subtitle.is_empty() && m.category == "Files" {
        let sub = Label::new(Some(&m.subtitle));
        sub.set_ellipsize(gtk4::pango::EllipsizeMode::Start);
        sub.set_max_width_chars(16);
        sub.set_halign(Align::Center);
        sub.add_css_class("grun-sub");
        main.append(&sub);
    }

    let card = GtkBox::new(Orientation::Horizontal, 12);
    card.add_css_class("grun-card");
    // Fixed size so the cards form a uniform grid (independent of button count)
    // and wrap into rows; the main area expands within that, and wrapping text
    // is bounded by it.
    card.set_size_request(CARD_W, CARD_H);
    card.set_hexpand(false);
    card.set_vexpand(false);
    card.append(&left);
    card.append(&main);
    (card, chips)
}

thread_local! {
    /// Decoded+scaled image textures, keyed by "path|WxH", so we don't re-read
    /// and re-scale the same image from disk on every keystroke.
    static TEXTURE_CACHE: RefCell<HashMap<String, gdk::Texture>> = RefCell::new(HashMap::new());
}

/// Load (or reuse) a scaled texture for a file path.
fn cached_texture(path: &str, w: i32, h: i32) -> Option<gdk::Texture> {
    let key = format!("{path}|{w}x{h}");
    TEXTURE_CACHE.with(|cache| {
        if let Some(t) = cache.borrow().get(&key) {
            return Some(t.clone());
        }
        let pb = gtk4::gdk_pixbuf::Pixbuf::from_file_at_scale(path, w, h, true).ok()?;
        let tex = gdk::Texture::for_pixbuf(&pb);
        let mut c = cache.borrow_mut();
        if c.len() > 256 {
            c.clear(); // simple bound for a long session
        }
        c.insert(key, tex.clone());
        Some(tex)
    })
}

/// Build an icon image from a Match's icon field: a file path (e.g. an AppImage
/// icon) loads from disk (cached); otherwise it's a themed icon name.
fn icon_image(name: Option<&str>, size: i32) -> Image {
    if let Some(s) = name {
        if s.starts_with('/') && std::path::Path::new(s).exists() {
            // SVGs (e.g. the AI icon) load directly so they render crisply at any
            // size; raster paths go through the scaled-texture cache.
            if s.to_lowercase().ends_with(".svg") {
                let img = Image::from_file(s);
                img.set_pixel_size(size);
                return img;
            }
            if let Some(tex) = cached_texture(s, size, size) {
                let img = Image::from_paintable(Some(&tex));
                img.set_pixel_size(size);
                return img;
            }
        }
    }
    let img = Image::from_icon_name(name.unwrap_or("application-x-executable"));
    img.set_pixel_size(size);
    img
}

/// A scaled image thumbnail from a file path (cached), or a generic icon.
fn thumbnail(path: &str) -> Image {
    // Sized to sit inside a fixed card's main area without widening it.
    match cached_texture(path, 120, 80) {
        Some(tex) => {
            let img = Image::from_paintable(Some(&tex));
            img.set_size_request(120, 80);
            img
        }
        None => {
            let img = Image::from_icon_name("image-x-generic");
            img.set_pixel_size(52);
            img
        }
    }
}

/// True if `path` looks like an image file by extension.
fn is_image_path(path: &str) -> bool {
    let p = path.to_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp", ".svg", ".ico"]
        .iter()
        .any(|e| p.ends_with(e))
}

/// If a clipboard string is a path/URI to an existing image file, return it.
fn clipboard_image_path(text: &str) -> Option<String> {
    let s = text.trim();
    let path = s.strip_prefix("file://").unwrap_or(s);
    if is_image_path(path) && std::path::Path::new(path).exists() {
        Some(path.to_string())
    } else {
        None
    }
}

/// Build one grouped result row: [letter] [icon] title/subtitle [action chips].
/// Returns the row widget and its chip buttons (for wiring secondary actions).
fn build_grouped_row(letter: char, m: &Match) -> (GtkBox, Vec<Button>) {
    let badge = Label::new(Some(&letter.to_string()));
    badge.add_css_class("grun-letter");
    badge.set_valign(Align::Center);

    let icon = icon_image(m.icon.as_deref(), 32);

    let text = GtkBox::new(Orientation::Vertical, 0);
    text.set_valign(Align::Center);
    text.set_hexpand(true);

    let title = Label::new(Some(&m.title));
    title.set_halign(Align::Start);
    title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    title.add_css_class("grun-title");

    // Title line carries an optional small package-type tag.
    let title_row = GtkBox::new(Orientation::Horizontal, 6);
    title_row.append(&title);
    if let Some(tag) = &m.tag {
        let badge = Label::new(Some(tag));
        badge.add_css_class("grun-tag");
        badge.add_css_class(tag_class(tag));
        badge.set_valign(Align::Center);
        title_row.append(&badge);
    }
    text.append(&title_row);

    if !m.subtitle.is_empty() {
        let sub = Label::new(Some(&m.subtitle));
        sub.set_halign(Align::Start);
        sub.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        sub.add_css_class("grun-sub");
        text.append(&sub);
    }

    let row = GtkBox::new(Orientation::Horizontal, 10);
    row.add_css_class("grun-row");
    row.append(&badge);
    row.append(&icon);
    row.append(&text);

    // Secondary action chips, numbered 1, 2, …
    let mut chips = Vec::new();
    for (n, (_id, label, _)) in m.actions.iter().enumerate() {
        let chip = Button::with_label(&format!("{} {}", n + 1, label));
        chip.add_css_class("grun-chip");
        chip.set_valign(Align::Center);
        row.append(&chip);
        chips.push(chip);
    }

    (row, chips)
}

fn load_css() {
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(CSS);
    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
