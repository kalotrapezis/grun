//! System power states (KRunner-style): typing "shutdown", "restart", "sleep",
//! "hibernate", "lock" or "log out" offers the matching action. Backed by
//! systemd (`systemctl` / `loginctl`), which lets an active local session run
//! these without sudo via logind/polkit.

use super::Provider;
use crate::matching::{Action, Match};

struct PowerAction {
    label: &'static str,
    /// Words that should surface this action.
    aliases: &'static [&'static str],
    /// Freedesktop/Mint themed icon name.
    icon: &'static str,
    /// The shell command run on activation (also shown as the subtitle).
    cmd: &'static str,
}

const ACTIONS: &[PowerAction] = &[
    PowerAction {
        label: "Power off",
        aliases: &["shutdown", "power off", "poweroff", "turn off"],
        icon: "system-shutdown",
        cmd: "systemctl poweroff",
    },
    PowerAction {
        label: "Restart",
        aliases: &["restart", "reboot"],
        icon: "system-reboot",
        cmd: "systemctl reboot",
    },
    PowerAction {
        label: "Sleep",
        aliases: &["sleep", "suspend"],
        icon: "system-suspend",
        cmd: "systemctl suspend",
    },
    PowerAction {
        label: "Hibernate",
        aliases: &["hibernate"],
        icon: "system-hibernate",
        cmd: "systemctl hibernate",
    },
    PowerAction {
        label: "Lock screen",
        aliases: &["lock", "lock screen"],
        icon: "system-lock-screen",
        cmd: "loginctl lock-session",
    },
    PowerAction {
        label: "Log out",
        aliases: &["log out", "logout", "sign out"],
        icon: "system-log-out",
        cmd: "loginctl terminate-session \"$XDG_SESSION_ID\"",
    },
];

pub struct PowerProvider;

impl Provider for PowerProvider {
    fn query(&self, input: &str) -> Vec<Match> {
        let q = input.trim();
        if q.chars().count() < 2 {
            return Vec::new();
        }
        let mut out = Vec::new();
        for a in ACTIONS {
            // Best relevance across this action's aliases; keep it tight so a
            // loose subsequence match doesn't pop a power action unexpectedly.
            let best = a
                .aliases
                .iter()
                .filter_map(|alias| crate::text::relevance(q, alias))
                .fold(0.0f32, f32::max);
            if best >= 0.6 {
                out.push(Match::new(
                    a.label.to_string(),
                    a.cmd.to_string(),
                    Some(a.icon.to_string()),
                    best,
                    "System",
                    Action::Shell(a.cmd.to_string()),
                ));
            }
        }
        out
    }
}
