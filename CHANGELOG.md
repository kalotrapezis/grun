# Changelog

All notable changes to grun are documented here. This project adheres to
[Semantic Versioning](https://semver.org/).

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
