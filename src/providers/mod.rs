//! The provider system. Each function is a `Provider`. The `Registry` builds
//! the enabled ones in priority order (from config) and merges their results:
//! a higher-priority function's results rank above a lower one's, and within a
//! function, by score. This is grun's equivalent of KRunner "runners".

use crate::config::Config;
use crate::matching::Match;
use crate::state::History;
use std::cell::RefCell;
use std::rc::Rc;

mod ai;
mod apps;
mod calc;
mod command;
mod files;
mod power;
mod search;

pub use ai::AiProvider;
pub use apps::AppsProvider;
pub use calc::CalcProvider;
pub use command::CommandProvider;
pub use files::FilesProvider;
pub use power::{label_for_cmd as power_label, PowerProvider};
pub use search::SearchProvider;

// Row builders reused by the empty-state dashboard.
pub use apps::make_match as app_to_match;
pub use files::path_to_match as file_to_match;

/// Max results contributed by a single function, so lower-priority functions
/// still get a chance to appear.
const PER_PROVIDER: usize = 6;
/// Max results shown overall.
const TOTAL: usize = 12;

pub trait Provider {
    /// Return matches for `input`. `input` is already trimmed and non-empty.
    fn query(&self, input: &str) -> Vec<Match>;

    /// Whether this provider should still run once the query has more than one
    /// word. App search is the heavy single-token case, so it opts out: after the
    /// first word the user is searching files / the web / AI, and skipping the
    /// per-keystroke scan of every installed app keeps multi-word queries snappy.
    fn wants_multiword(&self) -> bool {
        true
    }
}

fn make(id: &str, cfg: &Config, history: &Rc<RefCell<History>>) -> Option<Box<dyn Provider>> {
    Some(match id {
        "calc" => Box::new(CalcProvider),
        "apps" => Box::new(AppsProvider::new(cfg.search_descriptions, history.clone())),
        "files" => Box::new(FilesProvider::new()),
        "search" => Box::new(SearchProvider),
        "ai" => Box::new(AiProvider),
        "power" => Box::new(PowerProvider::new(history.clone())),
        "command" => Box::new(CommandProvider),
        _ => return None,
    })
}

/// Minimal percent-encoding for a URL query component (shared by the web search
/// and AI providers).
pub(crate) fn url_encode(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => o.push(b as char),
            b' ' => o.push('+'),
            _ => o.push_str(&format!("%{:02X}", b)),
        }
    }
    o
}

pub struct Registry {
    /// Enabled providers, already in priority order.
    providers: Vec<Box<dyn Provider>>,
}

impl Registry {
    pub fn from_config(cfg: &Config, history: &Rc<RefCell<History>>) -> Self {
        let providers = cfg
            .providers
            .iter()
            .filter(|p| p.enabled)
            .filter_map(|p| make(&p.id, cfg, history))
            .collect();
        Registry { providers }
    }

    /// Query every enabled provider in order, cap each, then order by
    /// (priority, score).
    pub fn query(&self, input: &str) -> Vec<Match> {
        let input = input.trim();
        if input.is_empty() {
            return Vec::new();
        }
        // Past the first word, skip providers that opt out (app search) so a long
        // query doesn't pay to scan every installed app; files, web search, AI,
        // power, calc and command keep running.
        let multiword = input.split_whitespace().count() > 1;
        // (priority index, match)
        let mut tagged: Vec<(usize, Match)> = Vec::new();
        for (idx, provider) in self.providers.iter().enumerate() {
            if multiword && !provider.wants_multiword() {
                continue;
            }
            let mut ms = provider.query(input);
            ms.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            ms.truncate(PER_PROVIDER);
            for m in ms {
                tagged.push((idx, m));
            }
        }
        tagged.sort_by(|a, b| {
            a.0.cmp(&b.0).then(
                b.1.score
                    .partial_cmp(&a.1.score)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
        });
        tagged.truncate(TOTAL);
        tagged.into_iter().map(|(_, m)| m).collect()
    }
}
