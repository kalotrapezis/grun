//! AI chat function. Hands whatever you typed to an AI assistant by opening its
//! web app with the query pre-filled (`?q=…`). Like the web-search function, the
//! default (Enter) assistant and the side actions come from the configurable
//! action list, so reordering in settings changes which one Enter opens.

use super::{url_encode, Provider};
use crate::matching::{Action, Match};

pub struct AiProvider;

impl Provider for AiProvider {
    fn query(&self, input: &str) -> Vec<Match> {
        let q = input.trim();
        if q.chars().count() < 2 {
            return Vec::new();
        }
        let enc = url_encode(q);

        // Each assistant opens its web app with the query pre-filled. Assistants
        // that don't honour a query parameter simply open to a fresh chat.
        let actions: Vec<(&'static str, String, Action)> = vec![
            (
                "claude",
                "Claude".to_string(),
                Action::OpenUrl(format!("https://claude.ai/new?q={enc}")),
            ),
            (
                "chatgpt",
                "ChatGPT".to_string(),
                Action::OpenUrl(format!("https://chatgpt.com/?q={enc}")),
            ),
            (
                "deepseek",
                "DeepSeek".to_string(),
                Action::OpenUrl(format!("https://chat.deepseek.com/?q={enc}")),
            ),
            (
                "mistral",
                "Mistral".to_string(),
                Action::OpenUrl(format!("https://chat.mistral.ai/chat?q={enc}")),
            ),
        ];

        vec![Match::new(
            format!("Ask AI: “{q}”"),
            "Press Enter to use Claude".to_string(),
            crate::ai_row_icon(),
            0.48,
            "AI",
            Action::OpenUrl(format!("https://claude.ai/new?q={enc}")), // default
        )
        .with_actions(actions)]
    }
}
