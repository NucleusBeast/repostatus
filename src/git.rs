use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{process::Command, time::timeout};

#[derive(Debug, Clone, Default)]
pub struct Status {
    pub branch: String,
    pub upstream: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    pub changed: usize,
    pub conflicts: bool,
    pub detached: bool,
    pub warning: Option<String>,
}

impl Status {
    pub fn incoming(&self) -> bool {
        self.behind > 0
    }
    pub fn outbound(&self) -> bool {
        self.ahead > 0 || self.changed > 0
    }
    pub fn can_pull(&self) -> bool {
        self.incoming() && self.changed == 0 && !self.detached && !self.conflicts && self.ahead == 0
    }
    pub fn can_push(&self) -> bool {
        self.ahead > 0
            && self.behind == 0
            && self.upstream.is_some()
            && !self.detached
            && !self.conflicts
    }
}

#[derive(Debug, Clone)]
pub enum Action {
    Refresh,
    Pull,
    Push,
    Commit(String),
}

/// Invoke Git directly (never through a shell), preserving credential helpers and SSH configuration.
async fn git(path: &Path, args: &[&str]) -> Result<String, String> {
    let mut command = Command::new("git");
    // GUI processes otherwise give console children their own visible window.
    #[cfg(target_os = "windows")]
    command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW

    command
        .arg("-C")
        .arg(path)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .stdin(Stdio::null())
        .kill_on_drop(true);
    let output = timeout(Duration::from_secs(90), command.output())
        .await
        .map_err(|_| {
            "Git timed out after 90 seconds. Check your network or credentials.".to_string()
        })?
        .map_err(|error| format!("Could not run Git: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if message.is_empty() {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        } else {
            message
        })
    }
}

pub async fn repository_root(path: PathBuf) -> Result<PathBuf, String> {
    let path = path
        .canonicalize()
        .map_err(|e| format!("Cannot open folder: {e}"))?;
    if git(&path, &["rev-parse", "--is-inside-work-tree"]).await? != "true" {
        return Err("Choose a working Git repository, not a bare repository.".into());
    }
    let root = git(&path, &["rev-parse", "--show-toplevel"]).await?;
    PathBuf::from(root)
        .canonicalize()
        .map_err(|e| e.to_string())
}

pub async fn inspect(path: &Path, fetch: bool) -> Result<Status, String> {
    // Read local state even when the remote is temporarily unreachable.
    let warning = if fetch {
        git(path, &["fetch", "--all", "--prune"]).await.err()
    } else {
        None
    };
    let porcelain = git(path, &["status", "--porcelain=v2", "--branch", "-z"]).await?;
    let mut status = parse_status(&porcelain);
    status.warning = warning;
    Ok(status)
}

pub fn parse_status(porcelain: &str) -> Status {
    let mut status = Status::default();
    let mut entries = porcelain.split('\0');
    while let Some(entry) = entries.next() {
        if let Some(branch) = entry.strip_prefix("# branch.head ") {
            status.branch = branch.to_owned();
            status.detached = branch == "(detached)";
        } else if let Some(upstream) = entry.strip_prefix("# branch.upstream ") {
            status.upstream = Some(upstream.to_owned());
        } else if let Some(counts) = entry.strip_prefix("# branch.ab ") {
            let mut parts = counts.split_whitespace();
            status.ahead = parts
                .next()
                .and_then(|s| s.strip_prefix('+'))
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            status.behind = parts
                .next()
                .and_then(|s| s.strip_prefix('-'))
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
        } else if entry.starts_with("1 ")
            || entry.starts_with("2 ")
            || entry.starts_with("? ")
            || entry.starts_with("u ")
        {
            status.changed += 1;
            status.conflicts |= entry.starts_with("u ");
            // Rename records have a second NUL-delimited path, not another status record.
            if entry.starts_with("2 ") {
                entries.next();
            }
        }
    }
    status
}

pub async fn perform(path: PathBuf, action: Action) -> Result<Status, String> {
    match action {
        Action::Refresh => return inspect(&path, true).await,
        Action::Pull => {
            let current = inspect(&path, true).await?;
            if let Some(warning) = &current.warning {
                return Err(warning.clone());
            }
            if !current.can_pull() {
                return Err("Pull requires a clean working tree and a branch that can fast-forward. Resolve local changes or divergence first.".into());
            }
            git(&path, &["pull", "--ff-only"]).await?;
        }
        Action::Push => {
            let current = inspect(&path, true).await?;
            if let Some(warning) = &current.warning {
                return Err(warning.clone());
            }
            if !current.can_push() {
                return Err("Push requires local commits, a configured upstream, and no incoming commits or conflicts.".into());
            }
            // Push exactly the upstream shown in the UI, independent of push.default / pushRemote.
            let branch = git(&path, &["symbolic-ref", "--quiet", "--short", "HEAD"]).await?;
            let remote = git(
                &path,
                &["config", "--get", &format!("branch.{branch}.remote")],
            )
            .await?;
            let target = git(
                &path,
                &["config", "--get", &format!("branch.{branch}.merge")],
            )
            .await?;
            let refspec = format!("HEAD:{target}");
            git(&path, &["push", "--", &remote, &refspec]).await?;
        }
        Action::Commit(message) => {
            if message.trim().is_empty() {
                return Err("Enter a commit message.".into());
            }
            let current = inspect(&path, false).await?;
            if current.conflicts || current.detached {
                return Err("Resolve conflicts and check out a branch before committing.".into());
            }
            git(&path, &["add", "--all"]).await?;
            git(&path, &["commit", "-m", message.trim()]).await?;
        }
    }
    inspect(&path, true).await
}
