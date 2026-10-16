# Changelog

Notable changes to RepoStatus are recorded here, grouped by version.

## [0.1.1] - 2026-10-16

### Added

- A saved Settings choice to hide the window or quit the app when closing it on
  macOS and Windows.
- Optional start at login on macOS, Windows, and Linux, controlled from Settings
  through the operating system's per-user startup registration.
- Clickable project paths that open repository folders in the system's default app.
- Saved default and per-project app choices for opening repository folders in an
  editor or another app, with options to restore the system default or inherit
  the default app.

### Changed

- Dashboard tooltips use themed panels with consistent padding and spacing.
- Settings dialogs have vertical margins to keep their contents accessible.
- Windows paths display with forward slashes and without verbatim path prefixes,
  while preserving drive and network roots.
- Tray icons, Windows window icons, and Windows executable icons use a black R
  glyph on a transparent background.
- Release builds run manually through GitHub Actions.
- README documents release downloads, project app choices, close behavior, and
  start-at-login behavior.

## [0.1.0] - 2026-10-13

### Added

- Native Rust and Iced desktop dashboard for monitoring multiple local Git
  repositories, with saved project lists and preferences.
- Repository status for branches, incoming and outgoing commits, changed files,
  conflicts, detached HEAD, missing upstreams, and fetch failures.
- Background repository checks on startup and every 60 seconds, plus manual refresh.
- Fast-forward pulls, commits, and pushes with safeguards for dirty working trees,
  divergence, and conflicts.
- Folder selection, project removal confirmation, keyboard shortcuts, and Git
  operation timeouts.
- Light, Dark, and System themes, window transparency on macOS and Windows, and
  optional CPU and memory usage in the footer.
- Menu bar and system tray integration on macOS and Windows, including background
  operation when the dashboard is closed.
- Restoration and focus of the existing dashboard when reopening the macOS app.
- Release builds for macOS and Windows on ARM64 and x64, with macOS signing and
  notarization support.
- Platform app icons and a static project website with app screenshots.
- Usage, configuration, troubleshooting, and release documentation.
- Automated tests for Git workflows, configuration persistence, and app behavior.

[0.1.1]: https://github.com/NucleusBeast/repostatus/releases/tag/v0.1.1
[0.1.0]: https://github.com/NucleusBeast/repostatus/releases/tag/v0.1.0
