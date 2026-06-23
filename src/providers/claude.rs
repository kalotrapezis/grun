//! Claude function. When enabled, offers to hand whatever you typed to Claude.
//!
//! NOTE: this is a placeholder integration — it copies the text and opens
//! claude.ai. The real behaviour (open Claude Code CLI, add to calendar, etc.)
//! is still being designed with the user.

use super::Provider;
use crate::matching::{Action, Match};

pub struct ClaudeProvider;

impl Provider for ClaudeProvider {
    fn query(&self, input: &str) -> Vec<Match> {
        let q = input.trim();
        if q.chars().count() < 2 {
            return Vec::new();
        }
        vec![Match::new(
            format!("Ask Claude: “{}”", q),
            "Copies the text and opens claude.ai".to_string(),
            Some("dialog-question".to_string()),
            0.45,
            "Claude",
            Action::Claude(q.to_string()),
        )]
    }
}
