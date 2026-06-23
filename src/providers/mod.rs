//! The provider system. Each function is a `Provider`. The `Registry` builds
//! the enabled ones in priority order (from config) and merges their results:
//! a higher-priority function's results rank above a lower one's, and within a
//! function, by score. This is grun's equivalent of KRunner "runners".

use crate::config::Config;
use crate::matching::Match;
use crate::state::History;
use std::cell::RefCell;
use std::rc::Rc;

mod apps;
mod calc;
mod claude;
mod command;
mod files;
mod google;

pub use apps::AppsProvider;
pub use calc::CalcProvider;
pub use claude::ClaudeProvider;
pub use command::CommandProvider;
pub use files::FilesProvider;
pub use google::GoogleProvider;

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
}

fn make(id: &str, cfg: &Config, history: &Rc<RefCell<History>>) -> Option<Box<dyn Provider>> {
    Some(match id {
        "calc" => Box::new(CalcProvider),
        "apps" => Box::new(AppsProvider::new(cfg.search_descriptions, history.clone())),
        "files" => Box::new(FilesProvider::new()),
        "google" => Box::new(GoogleProvider::new()),
        "claude" => Box::new(ClaudeProvider),
        "command" => Box::new(CommandProvider),
        _ => return None,
    })
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
        // (priority index, match)
        let mut tagged: Vec<(usize, Match)> = Vec::new();
        for (idx, provider) in self.providers.iter().enumerate() {
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
