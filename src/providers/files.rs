//! File search. Builds a bounded index of your home folder at startup, then
//! matches filenames with the same layout-aware, fuzzy/glob logic as apps.

use super::Provider;
use crate::matching::{Action, Match};
use gtk4::gio;
use gtk4::prelude::*;
use std::path::{Path, PathBuf};

const MAX_ENTRIES: usize = 60_000;
const MAX_DEPTH: usize = 8;

/// Directories we never descend into (noise / huge / slow).
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "__pycache__",
    ".git",
    ".cache",
    ".cargo",
    ".rustup",
    ".npm",
    ".mozilla",
    ".thunderbird",
    "snap",
    "venv",
    ".venv",
];

struct Entry {
    name: String,
    path: PathBuf,
    is_dir: bool,
}

pub struct FilesProvider {
    index: Vec<Entry>,
}

impl FilesProvider {
    pub fn new() -> Self {
        FilesProvider {
            index: build_index(),
        }
    }
}

impl Provider for FilesProvider {
    fn query(&self, input: &str) -> Vec<Match> {
        let q = input.trim();
        if q.chars().count() < 2 {
            return Vec::new();
        }
        let is_glob = q.contains('*');
        // Score cheaply first; only build the (icon-resolving) Match for the few
        // results we actually keep, so a broad query stays responsive.
        let mut scored: Vec<(f32, &Entry)> = Vec::new();
        for e in &self.index {
            let score = if is_glob {
                if crate::text::glob_match(q, &e.name) {
                    Some(0.8)
                } else {
                    None
                }
            } else {
                // Files rank just below apps at equal quality.
                crate::text::relevance(q, &e.name).map(|s| s * 0.8)
            };
            if let Some(score) = score {
                scored.push((score, e));
            }
        }
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(20);
        scored
            .into_iter()
            .map(|(score, e)| make_match(&e.path, e.is_dir, score))
            .collect()
    }
}

/// Build a file result: open by default, plus Copy path / Open in folder.
pub fn make_match(path: &std::path::Path, is_dir: bool, score: f32) -> Match {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string());
    Match::new(
        name,
        path.to_string_lossy().to_string(),
        Some(file_icon_name(path, is_dir)),
        score,
        "Files",
        Action::OpenPath(path.to_path_buf()),
    )
    .with_actions(vec![
        (
            "copy_path",
            "Copy path".to_string(),
            Action::Copy(path.to_string_lossy().to_string()),
        ),
        (
            "open_folder",
            "Open in folder".to_string(),
            Action::OpenLocation(path.to_path_buf()),
        ),
        ("hide", "Hide".to_string(), Action::HideFile(path.to_path_buf())),
    ])
}

/// Build a file result from a path on disk (used by the empty-state list).
pub fn path_to_match(path: &std::path::Path, score: f32) -> Match {
    let is_dir = path.is_dir();
    make_match(path, is_dir, score)
}

thread_local! {
    /// Cache of path → resolved icon name, so we don't re-guess the MIME type
    /// and re-query the icon theme for the same file on every keystroke.
    static MIME_ICON_CACHE: std::cell::RefCell<std::collections::HashMap<String, String>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// The real icon name for a file, from its MIME type (cached).
fn file_icon_name(path: &Path, is_dir: bool) -> String {
    if is_dir {
        return "folder".to_string();
    }
    let key = path.to_string_lossy().to_string();
    if let Some(v) = MIME_ICON_CACHE.with(|c| c.borrow().get(&key).cloned()) {
        return v;
    }
    let name = resolve_file_icon_name(path);
    MIME_ICON_CACHE.with(|c| {
        c.borrow_mut().insert(key, name.clone());
    });
    name
}

fn resolve_file_icon_name(path: &Path) -> String {
    let (content_type, _) = gio::content_type_guess(Some(path), &[]);
    let icon = gio::content_type_get_icon(content_type.as_str());
    if let Some(themed) = icon.downcast_ref::<gio::ThemedIcon>() {
        if let Some(display) = gtk4::gdk::Display::default() {
            let theme = gtk4::IconTheme::for_display(&display);
            for name in themed.names() {
                if theme.has_icon(&name) {
                    return name.to_string();
                }
            }
        }
        if let Some(first) = themed.names().first() {
            return first.to_string();
        }
    }
    "text-x-generic".to_string()
}

fn build_index() -> Vec<Entry> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };
    let mut index = Vec::new();
    let mut stack = vec![(home, 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        if index.len() >= MAX_ENTRIES {
            break;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            let path = entry.path();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            index.push(Entry {
                name,
                path: path.clone(),
                is_dir,
            });
            if is_dir && depth < MAX_DEPTH {
                stack.push((path, depth + 1));
            }
        }
    }
    index
}
