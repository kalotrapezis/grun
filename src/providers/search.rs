//! Web search function. Produces one "Search" result whose default (Enter)
//! action and side actions come from the configurable action list — so the user
//! can make DuckDuckGo the default by reordering it above Google in settings.
//! The actual primary/side split is applied later by `apply_action_prefs`.

use super::{url_encode, Provider};
use crate::matching::{Action, Match};

pub struct SearchProvider;

impl Provider for SearchProvider {
    fn query(&self, input: &str) -> Vec<Match> {
        let q = input.trim();
        if q.chars().count() < 2 {
            return Vec::new();
        }
        let enc = url_encode(q);
        let google = format!("https://www.google.com/search?q={enc}");
        let ddg = format!("https://duckduckgo.com/?q={enc}");
        let swisscows = format!("https://swisscows.com/en/web?query={enc}");

        // Full candidate set, in default order. `apply_action_prefs` reorders it
        // per config and promotes the first enabled one to the primary action.
        let actions: Vec<(&'static str, String, Action)> = vec![
            ("google", "Google".to_string(), Action::OpenUrl(google.clone())),
            ("duckduckgo", "DuckDuckGo".to_string(), Action::OpenUrl(ddg)),
            ("swisscows", "Swisscows".to_string(), Action::OpenUrl(swisscows)),
        ];

        vec![Match::new(
            format!("Search the web for “{q}”"),
            "Press Enter to use Google".to_string(),
            Some("system-search".to_string()),
            0.5,
            "Search",
            Action::OpenUrl(google), // default until prefs are applied
        )
        .with_actions(actions)]
    }
}
