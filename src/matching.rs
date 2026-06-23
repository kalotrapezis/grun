//! Core data types shared by every provider.
//!
//! This mirrors KRunner's idea of a `QueryMatch`: a provider turns the user's
//! query into a list of scored `Match`es, each carrying an `Action` to run when
//! the user picks it.

use gtk4::gio;
use gtk4::prelude::*;

/// A single result row.
pub struct Match {
    pub title: String,
    pub subtitle: String,
    /// Freedesktop icon name (e.g. "firefox"), resolved against the icon theme.
    pub icon: Option<String>,
    /// Higher = ranked closer to the top. Calculator hits use >1.0 so they win.
    pub score: f32,
    /// Section label shown in the grouped UI ("Apps", "Files", …).
    pub category: &'static str,
    /// Default action (Enter / row letter).
    pub action: Action,
    /// Extra actions shown as numbered chips (1, 2, …): (stable id, label, action).
    pub actions: Vec<(&'static str, String, Action)>,
    /// Small badge text (e.g. package type "Flatpak").
    pub tag: Option<String>,
}

impl Match {
    /// Convenience constructor; secondary actions and tag default to none.
    pub fn new(
        title: String,
        subtitle: String,
        icon: Option<String>,
        score: f32,
        category: &'static str,
        action: Action,
    ) -> Self {
        Match {
            title,
            subtitle,
            icon,
            score,
            category,
            action,
            actions: Vec::new(),
            tag: None,
        }
    }

    pub fn with_actions(mut self, actions: Vec<(&'static str, String, Action)>) -> Self {
        self.actions = actions;
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }
}

/// What happens when a match is activated.
pub enum Action {
    /// Launch an installed application.
    LaunchApp(gio::AppInfo),
    /// Copy text to the clipboard (used by the calculator).
    Copy(String),
    /// Open a URL in the default browser.
    OpenUrl(String),
    /// Open a file or folder with its default handler.
    OpenPath(std::path::PathBuf),
    /// Run text as a shell command.
    Shell(String),
    /// Hand text to Claude (copies it and opens claude.ai for now).
    Claude(String),
    /// Put a saved image back on the clipboard (via xclip).
    CopyImage(String),
    /// Open the folder containing a file.
    OpenLocation(std::path::PathBuf),
    /// Pin/unpin a clipboard entry (handled by the UI, which owns history).
    PinClip(String),
    /// Delete a clipboard entry (handled by the UI).
    RemoveClip(String),
    /// Hide a file from results — restorable later (handled by the UI).
    HideFile(std::path::PathBuf),
    /// Run a shell command inside a terminal (so the user sees output/confirms).
    TerminalRun(String),
    /// Copy text to the clipboard and launch an app (Claude Desktop "paste it in").
    ClipAndOpenApp { text: String, app: gio::AppInfo },
    /// Open the software manager — at the app's page where the manager supports
    /// it (gnome-software / Discover); otherwise just open the manager.
    OpenStore(String),
}

impl Action {
    pub fn run(&self) {
        match self {
            Action::LaunchApp(info) => {
                if let Err(e) = info.launch(&[], gio::AppLaunchContext::NONE) {
                    eprintln!("grun: failed to launch app: {e}");
                }
            }
            Action::Copy(text) => {
                if let Some(display) = gtk4::gdk::Display::default() {
                    display.clipboard().set_text(text);
                }
            }
            Action::OpenUrl(url) => {
                if let Err(e) = gio::AppInfo::launch_default_for_uri(url, gio::AppLaunchContext::NONE)
                {
                    eprintln!("grun: failed to open url: {e}");
                }
            }
            Action::OpenPath(path) => {
                let uri = gio::File::for_path(path).uri();
                if let Err(e) =
                    gio::AppInfo::launch_default_for_uri(&uri, gio::AppLaunchContext::NONE)
                {
                    eprintln!("grun: failed to open path: {e}");
                }
            }
            Action::Shell(cmd) => {
                if let Err(e) = std::process::Command::new("sh").arg("-c").arg(cmd).spawn() {
                    eprintln!("grun: failed to run command: {e}");
                }
            }
            Action::Claude(text) => {
                if let Some(display) = gtk4::gdk::Display::default() {
                    display.clipboard().set_text(text);
                }
                let _ =
                    gio::AppInfo::launch_default_for_uri("https://claude.ai/new", gio::AppLaunchContext::NONE);
            }
            Action::CopyImage(path) => {
                // xclip reads the file and serves it as image/png to pasters.
                if let Ok(data) = std::fs::read(path) {
                    if let Ok(mut child) = std::process::Command::new("xclip")
                        .args(["-selection", "clipboard", "-t", "image/png"])
                        .stdin(std::process::Stdio::piped())
                        .spawn()
                    {
                        use std::io::Write;
                        if let Some(stdin) = child.stdin.as_mut() {
                            let _ = stdin.write_all(&data);
                        }
                    }
                }
            }
            Action::OpenLocation(path) => {
                let dir = path.parent().unwrap_or(path);
                let uri = gio::File::for_path(dir).uri();
                if let Err(e) =
                    gio::AppInfo::launch_default_for_uri(&uri, gio::AppLaunchContext::NONE)
                {
                    eprintln!("grun: failed to open location: {e}");
                }
            }
            // These mutate history and are handled by the UI layer.
            Action::PinClip(_) | Action::RemoveClip(_) | Action::HideFile(_) => {}
            Action::TerminalRun(cmd) => spawn_in_terminal(cmd),
            Action::ClipAndOpenApp { text, app } => {
                if let Some(display) = gtk4::gdk::Display::default() {
                    display.clipboard().set_text(text);
                }
                let _ = app.launch(&[], gio::AppLaunchContext::NONE);
            }
            Action::OpenStore(id) => {
                use std::process::Command;
                // Prefer managers that can deep-link to the app's page.
                if Command::new("gnome-software")
                    .arg(format!("--details={id}"))
                    .spawn()
                    .is_ok()
                {
                    return;
                }
                if Command::new("plasma-discover")
                    .arg(format!("appstream://{id}"))
                    .spawn()
                    .is_ok()
                {
                    return;
                }
                // Mint's mintinstall can't deep-link; just open the manager.
                if Command::new("mintinstall").spawn().is_ok() {
                    return;
                }
                let _ = gio::AppInfo::launch_default_for_uri(
                    &format!("appstream://{id}"),
                    gio::AppLaunchContext::NONE,
                );
            }
        }
    }
}

/// Run `cmd` in whatever terminal emulator is available, keeping the window open
/// afterward so the user sees the result. Tries common terminals in order.
fn spawn_in_terminal(cmd: &str) {
    let full = format!("{cmd}; echo; read -n1 -r -p 'Press any key to close…' _");
    use std::process::Command;
    // (program, args-before-command). Different terminals use different flags.
    let candidates: [(&str, &[&str]); 5] = [
        ("x-terminal-emulator", &["-e", "bash", "-c"]),
        ("gnome-terminal", &["--", "bash", "-c"]),
        ("konsole", &["-e", "bash", "-c"]),
        ("xfce4-terminal", &["-x", "bash", "-c"]),
        ("xterm", &["-e", "bash", "-c"]),
    ];
    for (term, pre) in candidates {
        if std::process::Command::new(term)
            .args(pre)
            .arg(&full)
            .spawn()
            .is_ok()
        {
            return;
        }
    }
    let _ = Command::new("bash").arg("-c").arg(cmd).spawn(); // last resort: no terminal
}
