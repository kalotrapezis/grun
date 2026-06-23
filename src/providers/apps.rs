//! Application launcher provider. Enumerates installed `.desktop` apps via Gio
//! and fuzzy-matches them against the query.

use super::Provider;
use crate::matching::{Action, Match};
use crate::state::History;
use gtk4::gio;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub struct AppsProvider {
    apps: Vec<gio::AppInfo>,
    search_descriptions: bool,
    history: Rc<RefCell<History>>,
}

impl AppsProvider {
    pub fn new(search_descriptions: bool, history: Rc<RefCell<History>>) -> Self {
        let apps = gio::AppInfo::all()
            .into_iter()
            .filter(|a| a.should_show())
            .collect();
        AppsProvider {
            apps,
            search_descriptions,
            history,
        }
    }
}

impl Provider for AppsProvider {
    fn query(&self, input: &str) -> Vec<Match> {
        let mut out = Vec::new();
        for app in &self.apps {
            if let Some(score) = self.score(app, input) {
                out.push(make_match(app, score));
            }
        }
        out
    }
}

impl AppsProvider {
    /// Score an app: by name, and (if enabled) by description/keywords so e.g.
    /// "screenshot" finds Flameshot. Description matches rank below name matches.
    fn score(&self, app: &gio::AppInfo, input: &str) -> Option<f32> {
        let name = app.display_name().to_string();
        let mut best = crate::text::relevance(input, &name);
        if self.search_descriptions {
            let mut extra = app.description().map(|d| d.to_string()).unwrap_or_default();
            if let Some(desktop) = app.downcast_ref::<gio::DesktopAppInfo>() {
                if let Some(g) = desktop.generic_name() {
                    extra.push(' ');
                    extra.push_str(&g);
                }
                for kw in desktop.keywords() {
                    extra.push(' ');
                    extra.push_str(&kw);
                }
            }
            if let Some(s) = crate::text::keyword_match(input, &extra) {
                let s = (s * 0.7).min(0.6); // keep below name matches
                best = Some(best.map_or(s, |b| b.max(s)));
            }
        }
        // Tie-break by how often the app was launched through grun (read live),
        // so among equally-relevant names the one you actually use ranks first.
        best.map(|b| {
            let count = app
                .id()
                .map(|id| self.history.borrow().app_count(id.as_str()))
                .unwrap_or(0);
            b + (count as f32 * 0.02).min(0.15)
        })
    }
}

/// Build an app result row: launch by default, plus a package-type tag and the
/// Show details / Uninstall actions. Used by search and the empty-state.
pub fn make_match(app: &gio::AppInfo, score: f32) -> Match {
    let name = app.display_name().to_string();
    // Keep the original-case command for path lookups; lowercase for matching.
    let cmd_raw = app
        .commandline()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let cmd = cmd_raw.to_lowercase();
    let kind = classify_command(&cmd);

    // AppImages don't expose a theme icon; use the one beside the AppImage.
    let icon = if kind == "AppImage" {
        appimage_icon(&cmd_raw).or_else(|| icon_name(app))
    } else {
        icon_name(app)
    };

    let mut actions: Vec<(&'static str, String, Action)> = Vec::new();
    // "Show details" opens the software manager (at the app page where possible).
    if let Some(id) = component_id(app) {
        actions.push((
            "show_details",
            "Show details".to_string(),
            Action::OpenStore(id),
        ));
    }
    // Uninstall only where it's safe & derivable — Flatpak (no sudo, clean id).
    if kind == "Flatpak" {
        if let Some(id) = component_id(app) {
            actions.push((
                "uninstall",
                "Uninstall".to_string(),
                Action::TerminalRun(format!("flatpak uninstall {id}")),
            ));
        }
    }

    Match::new(
        name,
        app.description().map(|d| d.to_string()).unwrap_or_default(),
        icon,
        score,
        "Apps",
        Action::LaunchApp(app.clone()),
    )
    .with_tag(kind)
    .with_actions(actions)
}

/// Classify an app by its Exec command line.
fn classify_command(cmd: &str) -> &'static str {
    if cmd.contains("flatpak") {
        "Flatpak"
    } else if cmd.contains("/snap/") || cmd.contains("snap run") {
        "Snap"
    } else if cmd.contains(".appimage") {
        "AppImage"
    } else {
        "System"
    }
}

/// AppStream component id (used for the store URI and flatpak), derived from the
/// `.desktop` id.
fn component_id(app: &gio::AppInfo) -> Option<String> {
    let id = app.id()?.to_string();
    Some(id.trim_end_matches(".desktop").to_string())
}

/// Find an AppImage's icon next to it: `~/AppImages/.icons/<appname>` (the
/// user's convention), with or without a file extension.
fn appimage_icon(cmd_raw: &str) -> Option<String> {
    let token = cmd_raw
        .split_whitespace()
        .find(|t| t.to_lowercase().ends_with(".appimage"))?;
    let stem = std::path::Path::new(token)
        .file_stem()?
        .to_string_lossy()
        .to_string();
    let home = std::env::var_os("HOME")?;
    let dir = std::path::Path::new(&home).join("AppImages/.icons");
    let exact = dir.join(&stem);
    if exact.exists() {
        return Some(exact.to_string_lossy().to_string());
    }
    for ext in ["png", "svg", "jpg", "jpeg", "xpm", "ico"] {
        let p = dir.join(format!("{stem}.{ext}"));
        if p.exists() {
            return Some(p.to_string_lossy().to_string());
        }
    }
    None
}

/// Pull a freedesktop icon name out of an app's `GIcon`, if it's a themed icon.
fn icon_name(app: &gio::AppInfo) -> Option<String> {
    let icon = app.icon()?;
    let themed = icon.downcast_ref::<gio::ThemedIcon>()?;
    themed.names().first().map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::classify_command;

    #[test]
    fn classify_package_types() {
        assert_eq!(
            classify_command("/usr/bin/flatpak run org.mozilla.Thunderbird"),
            "Flatpak"
        );
        assert_eq!(classify_command("/snap/bin/spotify"), "Snap");
        assert_eq!(classify_command("snap run spotify"), "Snap");
        assert_eq!(
            classify_command("env desktopintegration=1 /home/teo/appimages/viber.appimage"),
            "AppImage"
        );
        assert_eq!(classify_command("/usr/bin/firefox"), "System");
    }
}
