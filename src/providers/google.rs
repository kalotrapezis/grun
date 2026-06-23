//! Search function. Default action is a Google search; secondary actions cover
//! DuckDuckGo and (if installed) handing the query to the Claude Desktop app.

use super::Provider;
use crate::matching::{Action, Match};
use gtk4::gio;
use gtk4::prelude::*;

pub struct GoogleProvider {
    /// The Claude Desktop app, if one is installed.
    claude: Option<gio::AppInfo>,
}

impl GoogleProvider {
    pub fn new() -> Self {
        let claude = gio::AppInfo::all()
            .into_iter()
            .find(|a| a.should_show() && a.display_name().to_lowercase().contains("claude"));
        GoogleProvider { claude }
    }
}

impl Provider for GoogleProvider {
    fn query(&self, input: &str) -> Vec<Match> {
        let q = input.trim();
        if q.chars().count() < 2 {
            return Vec::new();
        }

        let mut actions: Vec<(&'static str, String, Action)> = vec![(
            "duckduckgo",
            "DuckDuckGo".to_string(),
            Action::OpenUrl(format!("https://duckduckgo.com/?q={}", encode(q))),
        )];
        if let Some(app) = &self.claude {
            actions.push((
                "claude_desktop",
                "Claude Desktop".to_string(),
                Action::ClipAndOpenApp {
                    text: q.to_string(),
                    app: app.clone(),
                },
            ));
        }

        vec![Match::new(
            format!("Search Google for “{}”", q),
            "Press Enter to open in your browser".to_string(),
            Some("web-browser".to_string()),
            0.5,
            "Search",
            Action::OpenUrl(format!("https://www.google.com/search?q={}", encode(q))),
        )
        .with_actions(actions)]
    }
}

/// Minimal percent-encoding for a URL query component.
fn encode(s: &str) -> String {
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
