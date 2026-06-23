//! Command-execution function. When enabled, offers to run whatever you typed
//! as a shell command. Off by default (it always produces a result, and Enter
//! runs it), so the user opts in via settings.

use super::Provider;
use crate::matching::{Action, Match};

pub struct CommandProvider;

impl Provider for CommandProvider {
    fn query(&self, input: &str) -> Vec<Match> {
        let q = input.trim();
        if q.is_empty() {
            return Vec::new();
        }
        vec![Match::new(
            format!("Run: {}", q),
            "Execute as a shell command".to_string(),
            Some("utilities-terminal".to_string()),
            0.4,
            "Run command",
            Action::Shell(q.to_string()),
        )]
    }
}
