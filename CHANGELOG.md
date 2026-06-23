# Changelog

All notable changes to grun are documented here. This project adheres to
[Semantic Versioning](https://semver.org/).

## [0.0.4] — 2026-06-23

### Changed

- **The launcher no longer repositions its window.** GTK4 can't position windows
  on X11, so earlier versions moved the window via xdotool right after it mapped —
  which flickered on every open, and the tricks to mask that move (opacity) left
  the panel transparent or trailing a "broken-screen" line on muffin (Cinnamon).
  grun now lets the window manager place the window (it remembers the spot across
  open/close) and leaves it there: no flicker, no render glitches. The "Pop up at"
  and "Open on" settings are removed, since they only existed to drive that move.

## [0.0.3] — 2026-06-23

### Added

- **Hide apps from search** — each app result now has a **Hide** action that
  removes it from search entirely (a privacy feature). Restore hidden apps under
  Settings → "Hidden apps".
- **Hide system actions** — power/system actions (shutdown, sleep, …) get the
  same per-result **Hide** action, restorable under Settings → "Hidden system
  actions". Hide the ones you never use (e.g. Sleep).
- **Password-protect settings** — an option to require a polkit/sudo password
  prompt before the settings window opens. (Settings → Options.)

### Changed

- **Bigger full-screen search** — in the full-screen layout an active search now
  uses a larger search box and roomier result rows, so the extra space is easier
  on the eye.
- **Faster multi-word queries** — past the first word grun stops scanning every
  installed app (app search is single-word by nature), so longer queries stay
  snappy. File search also normalizes the query once up front instead of once
  per candidate.
- **Square window corners** — the launcher window no longer has rounded corners,
  which left transparent gaps that showed the desktop through and looked broken.

### Fixed

- **Settings window transparency** (packaging revision `0.0.3-1`) — the launcher's
  transparent surface (for the drop shadow) was leaking onto the settings window,
  rendering it see-through. The transparency is now scoped to the launcher only.

## [0.0.2] — 2026-06-23

### Added

- **AI chat function** — separate from web search, it opens an AI assistant's
  web app with your query pre-filled. Choices: Claude, ChatGPT, DeepSeek and
  Mistral.
- **Configurable default actions** — the Search and AI action lists are now
  *primary-selectable*: the top enabled entry is the Enter action and the rest
  are side actions, so you can make DuckDuckGo (or any assistant) the default
  just by reordering it in settings.
- **Swisscows** added as a web-search engine alongside Google and DuckDuckGo.
- **Hide from home** — each app and file card on the home dashboard now has a
  Hide action that removes just that suggestion from home (it stays searchable).
  Restore them under Settings → "Hidden from home".
- **Expandable home sections** — each home section (Clipboard / Apps / Files)
  has an expand/collapse toggle that reveals a second row of items.
- **Full-screen start-menu home** — a toolbar button switches the home into a
  full-screen layout: a clipboard row, as many app rows as fit the screen, and a
  files row. Active search is unchanged in either mode.
- **System power actions** — typing shutdown / restart / sleep / hibernate /
  lock / log out offers the matching action (systemd `systemctl`/`loginctl`).

### Changed

- **Window fits its content** — the window now grows from just the search box up
  to a maximum, sizing to whatever results are showing. Disabling a function or
  hiding items shrinks it instead of leaving empty space.
- **Follows the system theme** — colours now come from the active GTK theme, so
  grun matches the system light/dark mode and its Mint-Y accent colour. Section
  icons switch between light and dark variants to suit.
- The search box now shows an accent focus glow on open, and result rows are
  highlighted only once you start navigating (Tab/↓) — so it's clear the focus
  starts in the search box.

### Fixed

- The settings window can no longer be opened more than once; a second click
  focuses the existing window.
- Toggle switches in settings no longer stutter — the config save and registry
  rebuild are deferred so the switch animates immediately.

## [0.0.1] — 2026-06-23

First public release. 🎉

### Added

- **App launcher** — fuzzy search over installed `.desktop` apps with
  package-type tags (Flatpak / Snap / AppImage / System), Show details and
  Uninstall (Flatpak) actions, and AppImage icon support.
- **Layout-independent search** — matches across keyboard layouts (Greek ↔
  Latin) so the same keys find a result regardless of the active layout.
- **Typo-tolerant fuzzy matching** with prefix-aware scoring.
- **Clipboard manager** — background capture of text and images, pin/remove,
  with persistent history.
- **File search** — fuzzy filename search of the home folder, real MIME-type
  icons, image thumbnails, wildcards, plus Copy path / Open in folder / Hide.
- **Calculator**, **Google/DuckDuckGo search**, **run command**, and a
  **Claude** function.
- **Home dashboard** — recent clipboard, most-used/recent apps, and
  most-used/recent files in a grid.
- **Full keyboard navigation** — per-row letters, numbered per-row actions,
  navigation mode.
- **Settings** — reorder/disable functions and per-result actions, pop-up
  position, home dashboard sources, auto-focus delay, search-descriptions
  toggle, start-on-login, and a hidden-files manager.
- **Resident single-instance** design: bind `grun` to a hotkey for a show/hide
  toggle.
- Per-session **caches** for image textures and MIME icons for responsiveness.
- `.deb` packaging via `packaging/build-deb.sh`.

[0.0.1]: https://github.com/kalotrapezis/grun/releases/tag/v0.0.1
