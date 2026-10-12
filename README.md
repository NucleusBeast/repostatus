<img src="assets/icon.svg" width="64" height="64" alt="">

# RepoStatus

A native Rust + Iced dashboard for your local Git projects, with a compact terminal-inspired interface, monospace table rows, and keyboard shortcuts.

## Run

Install [Rust](https://rustup.rs/) (1.88 or newer) and Git, then:

```sh
cargo run --release
```

On Linux, the native window and folder picker require the usual X11/Wayland libraries and a working desktop portal (`xdg-desktop-portal` and a desktop-specific backend). macOS and Windows use native folder pickers.

## Use

- **Add Project**: browse for an existing repository or enter its path. Subfolders resolve to the repository root; duplicates are rejected.
- **Incoming**: remote commits need pulling. **Pull** fetches and uses `git pull --ff-only` when the working tree is clean and the branch can fast-forward.
- **Outbound**: local commits or uncommitted files need attention. **Commit…** explicitly stages all changed/untracked files and commits with your message. **Push** sends committed changes to the displayed upstream. Committing and pushing are separate actions.
- **Default**: a tracked branch has no incoming commits, outgoing commits, or local changes.
- **Remove**: removes a project from the dashboard, preserving the repository and its files.
- **Keyboard**: `a` adds a project, `r` refreshes, and `Esc` closes a dialog.
- **Window transparency (macOS / Windows)**: open **[settings]** and adjust transparency from 0% (opaque) to 50%. Enable **Keep text opaque** to fade the dashboard background while keeping text and controls opaque; dialogs stay opaque for readability. Leave it unchecked to fade the whole window, including text and the title bar. Changes apply immediately. The slider saves on release, and the checkbox saves on toggle; both are restored on startup. **Reset to opaque** sets transparency to 0%.
- **App stats**: the footer shows RepoStatus's CPU usage and resident memory in MiB, updated every 2 seconds. CPU usage is measured per core (100% means one fully used core and can exceed 100%). Git subprocesses are excluded. Toggle **Show CPU and memory usage** in **[settings]**; the preference is saved, and disabling it also stops stats collection. Stats are enabled by default.
- **Refresh**: fetches remote updates and checks local changes. Checks also run on startup and every 60 seconds, in the background.
- **Background operation (macOS / Windows)**: a RepoStatus icon appears in the macOS menu bar or Windows system tray. Closing the dashboard hides it and keeps repository checks and running Git operations active. On macOS, closing the window also removes the app from the Dock and app switcher. Left-clicking the tray icon toggles the dashboard: visible windows hide; hidden or minimized windows restore and receive focus. The Dock icon follows window visibility on macOS. Right-click opens the menu with **Toggle RepoStatus**, **Refresh repositories**, and **Quit RepoStatus**. If the tray icon cannot be created, the dashboard shows an error and closing it exits normally. On Linux, closing the window exits the app.

A project can show both Incoming and Outbound. Diverged branches, merge conflicts, detached HEAD, and missing upstreams are indicated explicitly. Resolve these in your normal Git workflow; the app never force-pushes, resets files, stashes changes, or creates automatic merge commits.

Git uses your existing SSH configuration and credential helpers. Terminal prompts are disabled so the UI stays responsive; configure credentials outside the app if a remote reports authentication errors. Operations have a 90-second timeout. Failed fetches show a warning alongside the last locally known status, rather than silently implying the remote is current. Failed actions show their Git error; Refresh rechecks the repository.

## Saved projects

The project list is stored in `repostatus/projects.json` under the operating system's configuration directory:

- macOS: `~/Library/Application Support/repostatus/projects.json`
- Linux: `~/.config/repostatus/projects.json`
- Windows: `%APPDATA%\repostatus\projects.json`

The same file also stores window transparency, the text opacity preference, and footer stats visibility. Existing project lists default to an opaque window and retain the whole-window fade behavior. Set `REPOSTATUS_CONFIG` to a custom JSON file path to use a separate project list and settings. An invalid configuration is reported and preserved for repair.

## Development

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Integration tests create isolated repositories and a local bare remote to exercise fetch, pull, commit, push, dirty/diverged branch protection, status parsing, and persistence. No network credentials or user repositories are needed.

Built with [Iced 0.14](https://docs.rs/iced/0.14.0/). Created in [T3 Code](https://t3.codes).
