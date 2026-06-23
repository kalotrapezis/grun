//! Persistent history: clipboard (text + images), app-launch stats, and recent
//! files. Mirrors the model in the user's voice project `clipboard.py`
//! (pinned-first, recency, eviction of unpinned overflow), persisted as JSON.

use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

/// Unpinned clips beyond this are evicted (oldest first). Pinned never evicted.
const MAX_UNPINNED_CLIPS: usize = 60;
const MAX_FILES: usize = 40;

#[derive(Serialize, Deserialize, Clone)]
pub struct ClipEntry {
    pub id: String,
    /// "text" or "image".
    pub kind: String,
    /// Text content (empty for images).
    pub text: String,
    /// Image file path (empty for text).
    pub path: String,
    pub pinned: bool,
    pub hidden: bool,
    /// Recency (unix seconds).
    pub ts: u64,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct AppStat {
    pub id: String,
    pub count: u32,
    pub last: u64,
}

#[derive(Serialize, Deserialize, Default)]
pub struct History {
    #[serde(default)]
    pub clips: Vec<ClipEntry>,
    #[serde(default)]
    pub apps: Vec<AppStat>,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub file_uses: Vec<AppStat>,
    #[serde(default)]
    pub hidden_files: Vec<String>,
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn hash_str(s: &str) -> String {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    format!("{:016x}", h.finish())
}

pub fn hash_bytes(b: &[u8]) -> String {
    let mut h = DefaultHasher::new();
    b.hash(&mut h);
    format!("{:016x}", h.finish())
}

fn data_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    Some(base.join("grun"))
}

fn history_path() -> Option<PathBuf> {
    Some(data_dir()?.join("history.json"))
}

/// Directory for saved clipboard images.
pub fn clips_dir() -> Option<PathBuf> {
    let d = data_dir()?.join("clips");
    let _ = std::fs::create_dir_all(&d);
    Some(d)
}

// Some accessors (pin/hide/top_apps/recent_files) are consumed by the grouped
// UI that lands next; allow them ahead of that.
#[allow(dead_code)]
impl History {
    pub fn load() -> Self {
        let Some(path) = history_path() else {
            return History::default();
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return History::default();
        };
        serde_json::from_str(&text).unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = history_path() else {
            return;
        };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, json);
        }
    }

    // ---------- clipboard ----------

    /// Add a clip (or bump recency if already present). `id` dedups.
    pub fn add_clip(&mut self, kind: &str, text: &str, path: &str) {
        let id = match kind {
            "image" => hash_str(path),
            _ => hash_str(text),
        };
        if let Some(existing) = self.clips.iter_mut().find(|c| c.id == id) {
            existing.ts = now();
            return;
        }
        self.clips.push(ClipEntry {
            id,
            kind: kind.to_string(),
            text: text.to_string(),
            path: path.to_string(),
            pinned: false,
            hidden: false,
            ts: now(),
        });
        self.evict_clips();
    }

    fn evict_clips(&mut self) {
        // Sort newest-first, keep all pinned + the newest MAX_UNPINNED_CLIPS others.
        self.clips.sort_by(|a, b| b.ts.cmp(&a.ts));
        let mut unpinned = 0;
        self.clips.retain(|c| {
            if c.pinned {
                return true;
            }
            unpinned += 1;
            unpinned <= MAX_UNPINNED_CLIPS
        });
    }

    /// Clips to show: not hidden, pinned first, newest within each group.
    pub fn visible_clips(&self) -> Vec<ClipEntry> {
        let mut pinned: Vec<ClipEntry> =
            self.clips.iter().filter(|c| c.pinned && !c.hidden).cloned().collect();
        let mut rest: Vec<ClipEntry> =
            self.clips.iter().filter(|c| !c.pinned && !c.hidden).cloned().collect();
        pinned.sort_by(|a, b| b.ts.cmp(&a.ts));
        rest.sort_by(|a, b| b.ts.cmp(&a.ts));
        pinned.extend(rest);
        pinned
    }

    pub fn set_pinned(&mut self, id: &str, pinned: bool) {
        if let Some(c) = self.clips.iter_mut().find(|c| c.id == id) {
            c.pinned = pinned;
        }
    }

    pub fn set_hidden(&mut self, id: &str, hidden: bool) {
        if let Some(c) = self.clips.iter_mut().find(|c| c.id == id) {
            c.hidden = hidden;
        }
    }

    /// Delete a clipboard entry outright.
    pub fn remove_clip(&mut self, id: &str) {
        self.clips.retain(|c| c.id != id);
    }

    // ---------- hidden files ----------

    pub fn hide_file(&mut self, path: &str) {
        if !self.hidden_files.iter().any(|p| p == path) {
            self.hidden_files.push(path.to_string());
        }
    }

    pub fn unhide_file(&mut self, path: &str) {
        self.hidden_files.retain(|p| p != path);
    }

    pub fn is_file_hidden(&self, path: &str) -> bool {
        self.hidden_files.iter().any(|p| p == path)
    }

    pub fn hidden_files(&self) -> Vec<String> {
        self.hidden_files.clone()
    }

    // ---------- apps ----------

    pub fn record_app_launch(&mut self, id: &str) {
        if let Some(a) = self.apps.iter_mut().find(|a| a.id == id) {
            a.count += 1;
            a.last = now();
        } else {
            self.apps.push(AppStat {
                id: id.to_string(),
                count: 1,
                last: now(),
            });
        }
    }

    /// Most-used app ids (ties broken by recency).
    pub fn top_apps(&self, n: usize) -> Vec<String> {
        let mut apps = self.apps.clone();
        apps.sort_by(|a, b| b.count.cmp(&a.count).then(b.last.cmp(&a.last)));
        apps.into_iter().take(n).map(|a| a.id).collect()
    }

    /// Launch count for an app id (0 if never launched through grun).
    pub fn app_count(&self, id: &str) -> u32 {
        self.apps.iter().find(|a| a.id == id).map(|a| a.count).unwrap_or(0)
    }

    /// Most-recently-launched app ids.
    pub fn recent_apps(&self, n: usize) -> Vec<String> {
        let mut apps = self.apps.clone();
        apps.sort_by(|a, b| b.last.cmp(&a.last));
        apps.into_iter().take(n).map(|a| a.id).collect()
    }

    // ---------- files ----------

    pub fn record_file(&mut self, path: &str) {
        self.files.retain(|p| p != path);
        self.files.insert(0, path.to_string());
        self.files.truncate(MAX_FILES);
        if let Some(f) = self.file_uses.iter_mut().find(|f| f.id == path) {
            f.count += 1;
            f.last = now();
        } else {
            self.file_uses.push(AppStat {
                id: path.to_string(),
                count: 1,
                last: now(),
            });
        }
    }

    pub fn recent_files(&self, n: usize) -> Vec<String> {
        self.files.iter().take(n).cloned().collect()
    }

    /// Most-used file paths (ties broken by recency).
    pub fn most_used_files(&self, n: usize) -> Vec<String> {
        let mut f = self.file_uses.clone();
        f.sort_by(|a, b| b.count.cmp(&a.count).then(b.last.cmp(&a.last)));
        f.into_iter().take(n).map(|x| x.id).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_dedup_bumps_recency_not_count() {
        let mut h = History::default();
        h.add_clip("text", "hello", "");
        h.add_clip("text", "world", "");
        h.add_clip("text", "hello", ""); // dup
        assert_eq!(h.clips.len(), 2);
    }

    #[test]
    fn pinned_sort_first_and_hidden_excluded() {
        let mut h = History::default();
        h.add_clip("text", "a", "");
        h.add_clip("text", "b", "");
        h.add_clip("text", "c", "");
        let cid = hash_str("c");
        let bid = hash_str("b");
        h.set_pinned(&cid, true);
        h.set_hidden(&bid, true);
        let vis = h.visible_clips();
        assert_eq!(vis[0].text, "c"); // pinned first
        assert!(vis.iter().all(|c| c.text != "b")); // hidden gone
        assert_eq!(vis.len(), 2);
    }

    #[test]
    fn app_launch_counts_and_ranks() {
        let mut h = History::default();
        h.record_app_launch("firefox");
        h.record_app_launch("firefox");
        h.record_app_launch("gedit");
        assert_eq!(h.top_apps(1), vec!["firefox".to_string()]);
    }

    #[test]
    fn remove_clip_deletes_and_hidden_files_roundtrip() {
        let mut h = History::default();
        h.add_clip("text", "keep", "");
        h.add_clip("text", "gone", "");
        h.remove_clip(&hash_str("gone"));
        assert_eq!(h.clips.len(), 1);
        assert_eq!(h.clips[0].text, "keep");

        h.hide_file("/a/b.txt");
        assert!(h.is_file_hidden("/a/b.txt"));
        assert_eq!(h.hidden_files(), vec!["/a/b.txt".to_string()]);
        h.unhide_file("/a/b.txt");
        assert!(!h.is_file_hidden("/a/b.txt"));
    }

    #[test]
    fn recent_files_move_to_front_dedup() {
        let mut h = History::default();
        h.record_file("/a");
        h.record_file("/b");
        h.record_file("/a");
        assert_eq!(h.recent_files(2), vec!["/a".to_string(), "/b".to_string()]);
    }
}
