//! Config: an *ordered* list of functions ("providers"), each enabled or not.
//! Order = priority (top of the list shows first). Stored one `id=on|off` per
//! line in `~/.config/grun/config`; the line order IS the priority order.

use std::fs;
use std::path::PathBuf;

/// All known functions, in their default order, with default enabled state.
/// `command` and `claude` default off (they always produce a result, so the
/// user opts in).
const KNOWN: &[(&str, bool)] = &[
    ("calc", true),
    ("apps", true),
    ("files", true),
    ("search", true),
    ("ai", true),
    ("power", true),
    ("command", false),
];

/// Known secondary actions per category, in default order: (id, label).
///
/// For the "Search" and "AI" categories the list is *primary-selectable*: the
/// first enabled action becomes the default (Enter) action and the rest show as
/// numbered side actions. So reordering this list in settings changes which
/// engine/assistant runs by default (e.g. put DuckDuckGo above Google).
pub const KNOWN_ACTIONS: &[(&str, &[(&str, &str)])] = &[
    ("Apps", &[("show_details", "Show details"), ("uninstall", "Uninstall")]),
    (
        "Files",
        &[
            ("copy_path", "Copy path"),
            ("open_folder", "Open in folder"),
            ("hide", "Hide"),
        ],
    ),
    (
        "Search",
        &[
            ("google", "Google"),
            ("duckduckgo", "DuckDuckGo"),
            ("swisscows", "Swisscows"),
        ],
    ),
    (
        "AI",
        &[
            ("claude", "Claude"),
            ("chatgpt", "ChatGPT"),
            ("deepseek", "DeepSeek"),
            ("mistral", "Mistral"),
        ],
    ),
    ("Clipboard", &[("pin", "Pin"), ("remove", "Remove")]),
];

/// Categories whose action list selects the default (Enter) action from its
/// first enabled entry, rather than having a fixed primary action.
pub fn primary_selectable(category: &str) -> bool {
    matches!(category, "Search" | "AI")
}

/// Human-readable label for an action id.
pub fn action_label(id: &str) -> &'static str {
    for (_, acts) in KNOWN_ACTIONS {
        for (aid, label) in *acts {
            if *aid == id {
                return label;
            }
        }
    }
    "Action"
}

#[derive(Clone)]
pub struct ActionPref {
    pub id: String,
    pub enabled: bool,
}

#[derive(Clone)]
pub struct CatActions {
    pub category: String,
    pub items: Vec<ActionPref>,
}

fn default_action_prefs() -> Vec<CatActions> {
    KNOWN_ACTIONS
        .iter()
        .map(|(cat, acts)| CatActions {
            category: cat.to_string(),
            items: acts
                .iter()
                .map(|(id, _)| ActionPref {
                    id: id.to_string(),
                    enabled: true,
                })
                .collect(),
        })
        .collect()
}

/// Human-readable label for a function id (used in the settings UI).
pub fn label(id: &str) -> &'static str {
    match id {
        "calc" => "Calculator",
        "apps" => "Apps",
        "files" => "Files",
        "search" => "Web search",
        "ai" => "AI chat",
        "power" => "System power",
        "command" => "Command execution",
        _ => "Unknown",
    }
}

#[derive(Clone)]
pub struct ProviderCfg {
    pub id: String,
    pub enabled: bool,
}

#[derive(Clone)]
pub struct Config {
    pub providers: Vec<ProviderCfg>,
    /// Where the window pops up: "center", "top", or "bottom".
    pub position: String,
    /// Show the clipboard section on the home/dashboard.
    pub home_clipboard: bool,
    /// Home apps section ordering: "used" or "recent".
    pub home_apps_mode: String,
    /// Home files section ordering: "recent" or "used".
    pub home_files_mode: String,
    /// Auto-focus the result list this many ms after typing (0 = off).
    pub focus_delay_ms: u32,
    /// Match apps on their description/keywords too, not just the name.
    pub search_descriptions: bool,
    /// Full-screen "start menu" home layout (home screen only; search unchanged).
    pub fullscreen: bool,
    /// Require a polkit/sudo password prompt before the settings window opens.
    pub lock_settings: bool,
    /// Per-category secondary-action order + enabled state.
    pub action_prefs: Vec<CatActions>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            providers: KNOWN
                .iter()
                .map(|(id, on)| ProviderCfg {
                    id: id.to_string(),
                    enabled: *on,
                })
                .collect(),
            position: "center".to_string(),
            home_clipboard: true,
            home_apps_mode: "used".to_string(),
            home_files_mode: "recent".to_string(),
            focus_delay_ms: 0,
            search_descriptions: false,
            fullscreen: false,
            lock_settings: false,
            action_prefs: default_action_prefs(),
        }
    }
}

fn config_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("grun").join("config"))
}

impl Config {
    pub fn load() -> Self {
        let Some(path) = config_path() else {
            return Config::default();
        };
        let Ok(text) = fs::read_to_string(&path) else {
            return Config::default();
        };

        let mut providers: Vec<ProviderCfg> = Vec::new();
        let mut cfg = Config::default();
        cfg.providers.clear();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                let key = k.trim().to_string();
                let val = v.trim();
                match key.as_str() {
                    "position" => {
                        if matches!(val, "center" | "top" | "bottom") {
                            cfg.position = val.to_string();
                        }
                        continue;
                    }
                    "home_clipboard" => {
                        cfg.home_clipboard = matches!(val, "on" | "true" | "1" | "yes");
                        continue;
                    }
                    "home_apps_mode" => {
                        if matches!(val, "used" | "recent") {
                            cfg.home_apps_mode = val.to_string();
                        }
                        continue;
                    }
                    "home_files_mode" => {
                        if matches!(val, "used" | "recent") {
                            cfg.home_files_mode = val.to_string();
                        }
                        continue;
                    }
                    "focus_delay_ms" => {
                        cfg.focus_delay_ms = val.parse().unwrap_or(0);
                        continue;
                    }
                    "search_descriptions" => {
                        cfg.search_descriptions = matches!(val, "on" | "true" | "1" | "yes");
                        continue;
                    }
                    "fullscreen" => {
                        cfg.fullscreen = matches!(val, "on" | "true" | "1" | "yes");
                        continue;
                    }
                    "lock_settings" => {
                        cfg.lock_settings = matches!(val, "on" | "true" | "1" | "yes");
                        continue;
                    }
                    _ => {}
                }
                // actions.<category>=id:on,id:off,…
                if let Some(cat) = key.strip_prefix("actions.") {
                    let items = val
                        .split(',')
                        .filter_map(|tok| tok.split_once(':'))
                        .map(|(id, on)| ActionPref {
                            id: id.trim().to_string(),
                            enabled: matches!(on.trim(), "on" | "true" | "1" | "yes"),
                        })
                        .collect::<Vec<_>>();
                    if !items.is_empty() {
                        if let Some(c) = cfg.action_prefs.iter_mut().find(|c| c.category == cat) {
                            // Merge: keep saved order/state for ids we still know,
                            // drop any that are gone (e.g. an old "claude_desktop"),
                            // then append any new known ids.
                            let known = known_action_ids(cat);
                            let mut merged: Vec<ActionPref> = items
                                .into_iter()
                                .filter(|p| known.iter().any(|&k| k == p.id.as_str()))
                                .collect();
                            for &k in &known {
                                if !merged.iter().any(|p| p.id.as_str() == k) {
                                    merged.push(ActionPref {
                                        id: k.to_string(),
                                        enabled: true,
                                    });
                                }
                            }
                            c.items = merged;
                        }
                    }
                    continue;
                }
                // Keep only provider ids we recognise, preserving file order.
                if KNOWN.iter().any(|(known, _)| *known == key)
                    && !providers.iter().any(|p| p.id == key)
                {
                    let enabled = matches!(val, "on" | "true" | "1" | "yes");
                    providers.push(ProviderCfg { id: key, enabled });
                }
            }
        }
        // Append any known functions the file didn't mention, with defaults.
        for (id, on) in KNOWN {
            if !providers.iter().any(|p| p.id == *id) {
                providers.push(ProviderCfg {
                    id: id.to_string(),
                    enabled: *on,
                });
            }
        }
        cfg.providers = providers;
        cfg
    }

    pub fn save(&self) {
        let Some(path) = config_path() else {
            return;
        };
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let mut body = String::from("# grun functions, in priority order\n");
        for p in &self.providers {
            body.push_str(&format!("{}={}\n", p.id, if p.enabled { "on" } else { "off" }));
        }
        body.push_str(&format!("position={}\n", self.position));
        body.push_str(&format!(
            "home_clipboard={}\n",
            if self.home_clipboard { "on" } else { "off" }
        ));
        body.push_str(&format!("home_apps_mode={}\n", self.home_apps_mode));
        body.push_str(&format!("home_files_mode={}\n", self.home_files_mode));
        body.push_str(&format!("focus_delay_ms={}\n", self.focus_delay_ms));
        body.push_str(&format!(
            "search_descriptions={}\n",
            if self.search_descriptions { "on" } else { "off" }
        ));
        body.push_str(&format!(
            "fullscreen={}\n",
            if self.fullscreen { "on" } else { "off" }
        ));
        body.push_str(&format!(
            "lock_settings={}\n",
            if self.lock_settings { "on" } else { "off" }
        ));
        for c in &self.action_prefs {
            let items = c
                .items
                .iter()
                .map(|p| format!("{}:{}", p.id, if p.enabled { "on" } else { "off" }))
                .collect::<Vec<_>>()
                .join(",");
            body.push_str(&format!("actions.{}={}\n", c.category, items));
        }
        let _ = fs::write(&path, body);
    }

    /// Move the item at `idx` up (delta -1) or down (delta +1), clamped.
    pub fn move_item(&mut self, idx: usize, delta: i32) {
        let target = idx as i32 + delta;
        if target < 0 || target as usize >= self.providers.len() {
            return;
        }
        self.providers.swap(idx, target as usize);
    }

    pub fn set_enabled(&mut self, idx: usize, on: bool) {
        if let Some(p) = self.providers.get_mut(idx) {
            p.enabled = on;
        }
    }

    pub fn set_position(&mut self, position: &str) {
        if matches!(position, "center" | "top" | "bottom") {
            self.position = position.to_string();
        }
    }

    /// Action prefs (id, enabled) for a category, in display order.
    pub fn action_order(&self, category: &str) -> Vec<(String, bool)> {
        self.action_prefs
            .iter()
            .find(|c| c.category == category)
            .map(|c| c.items.iter().map(|p| (p.id.clone(), p.enabled)).collect())
            .unwrap_or_default()
    }

    pub fn move_action(&mut self, category: &str, idx: usize, delta: i32) {
        if let Some(c) = self.action_prefs.iter_mut().find(|c| c.category == category) {
            let target = idx as i32 + delta;
            if target >= 0 && (target as usize) < c.items.len() {
                c.items.swap(idx, target as usize);
            }
        }
    }

    pub fn set_action_enabled(&mut self, category: &str, idx: usize, on: bool) {
        if let Some(c) = self.action_prefs.iter_mut().find(|c| c.category == category) {
            if let Some(p) = c.items.get_mut(idx) {
                p.enabled = on;
            }
        }
    }
}

/// The known action ids for a category, in default order.
fn known_action_ids(category: &str) -> Vec<&'static str> {
    KNOWN_ACTIONS
        .iter()
        .find(|(cat, _)| *cat == category)
        .map(|(_, acts)| acts.iter().map(|(id, _)| *id).collect())
        .unwrap_or_default()
}
