use repostatus::{
    git::{self, Action},
    storage,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::TempDir;

fn run(path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
fn identity(path: &Path) {
    run(path, &["config", "user.name", "RepoStatus Test"]);
    run(path, &["config", "user.email", "test@example.invalid"]);
    run(path, &["config", "commit.gpgsign", "false"]);
}
fn commit(path: &Path, filename: &str) {
    fs::write(path.join(filename), filename).unwrap();
    run(path, &["add", "--all"]);
    run(path, &["commit", "-m", filename]);
}
struct Fixture {
    _temp: TempDir,
    a: PathBuf,
    b: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let temp = TempDir::new().unwrap();
        let remote = temp.path().join("remote.git");
        let a = temp.path().join("project a");
        let b = temp.path().join("project b");
        fs::create_dir(&remote).unwrap();
        run(&remote, &["init", "--bare", "--initial-branch=main"]);
        fs::create_dir(&a).unwrap();
        run(&a, &["init", "--initial-branch=main"]);
        identity(&a);
        commit(&a, "initial.txt");
        run(&a, &["remote", "add", "origin", remote.to_str().unwrap()]);
        run(&a, &["push", "--set-upstream", "origin", "main"]);
        run(
            temp.path(),
            &["clone", remote.to_str().unwrap(), b.to_str().unwrap()],
        );
        identity(&b);
        Self { _temp: temp, a, b }
    }
}

#[tokio::test]
async fn default_incoming_pull_commit_outbound_push() {
    let f = Fixture::new();
    let s = git::inspect(&f.a, true).await.unwrap();
    assert!(!s.incoming() && !s.outbound());
    assert_eq!(s.branch, "main");
    commit(&f.b, "remote.txt");
    run(&f.b, &["push"]);
    let s = git::inspect(&f.a, true).await.unwrap();
    assert_eq!(s.behind, 1);
    assert!(s.can_pull());
    let s = git::perform(f.a.clone(), Action::Pull).await.unwrap();
    assert_eq!(s.behind, 0);
    assert!(f.a.join("remote.txt").exists());
    fs::write(f.a.join("local.txt"), "local change").unwrap();
    let s = git::inspect(&f.a, false).await.unwrap();
    assert_eq!(s.changed, 1);
    assert!(s.outbound());
    assert!(!s.can_push());
    let s = git::perform(f.a.clone(), Action::Commit("Local work".into()))
        .await
        .unwrap();
    assert_eq!(s.changed, 0);
    assert_eq!(s.ahead, 1);
    assert!(s.can_push());
    run(&f.a, &["config", "push.default", "nothing"]);
    run(&f.a, &["config", "branch.main.pushRemote", "nonexistent"]);
    let s = git::perform(f.a.clone(), Action::Push).await.unwrap();
    assert!(!s.incoming() && !s.outbound());
    git::perform(f.b.clone(), Action::Pull).await.unwrap();
    assert!(f.b.join("local.txt").exists());
}

#[tokio::test]
async fn dirty_and_diverged_branches_cannot_pull_or_push() {
    let f = Fixture::new();
    commit(&f.b, "remote.txt");
    run(&f.b, &["push"]);
    fs::write(f.a.join("unsaved.txt"), "preserve me").unwrap();
    let s = git::inspect(&f.a, true).await.unwrap();
    assert!(s.incoming() && s.outbound());
    assert!(!s.can_pull());
    assert!(git::perform(f.a.clone(), Action::Pull).await.is_err());
    assert_eq!(
        fs::read_to_string(f.a.join("unsaved.txt")).unwrap(),
        "preserve me"
    );
    commit(&f.a, "local.txt");
    let s = git::inspect(&f.a, true).await.unwrap();
    assert_eq!((s.ahead, s.behind), (1, 1));
    assert!(!s.can_pull() && !s.can_push());
    assert!(git::perform(f.a.clone(), Action::Push).await.is_err());
}

#[tokio::test]
async fn missing_upstream_detached_missing_repo_and_failed_fetch() {
    let f = Fixture::new();
    run(&f.a, &["branch", "--unset-upstream"]);
    let s = git::inspect(&f.a, false).await.unwrap();
    assert!(s.upstream.is_none());
    assert!(!s.can_push() && !s.can_pull());
    run(&f.a, &["checkout", "--detach"]);
    assert!(git::inspect(&f.a, false).await.unwrap().detached);
    run(
        &f.b,
        &[
            "remote",
            "set-url",
            "origin",
            "/nonexistent/repostatus-test-remote",
        ],
    );
    let s = git::inspect(&f.b, true).await.unwrap();
    assert!(s.warning.is_some());
    assert_eq!(s.branch, "main");
    assert!(git::inspect(&f.a.join("missing"), false).await.is_err());
    assert!(git::repository_root(f.a.join("initial.txt")).await.is_err());
    fs::create_dir(f.a.join("subfolder")).unwrap();
    assert_eq!(
        git::repository_root(f.a.join("subfolder")).await.unwrap(),
        f.a.canonicalize().unwrap()
    );
}

#[test]
fn porcelain_handles_renames_conflicts_and_unusual_names() {
    let s = git::parse_status(
        "# branch.head main\0# branch.upstream origin/main\0# branch.ab +2 -3\x002 R. N... 100644 100644 100644 a b R100 renamed\0? old-name\0? new-file\0u UU N... 100644 100644 100644 100644 a b c conflict\0",
    );
    assert_eq!((s.ahead, s.behind, s.changed), (2, 3, 3));
    assert!(s.conflicts);
}

#[test]
fn project_list_roundtrips_and_removal_preserves_files() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path().join("project");
    fs::create_dir(&repo).unwrap();
    fs::write(repo.join("keep.txt"), "keep").unwrap();
    let config = temp.path().join("config/projects.json");
    assert!(storage::load(&config).unwrap().is_empty());
    storage::save(&config, vec![repo.clone()]).unwrap();
    assert_eq!(storage::load(&config).unwrap(), vec![repo.clone()]);
    storage::save(&config, vec![]).unwrap();
    assert!(storage::load(&config).unwrap().is_empty());
    assert!(repo.join("keep.txt").exists());
    fs::write(&config, "invalid JSON").unwrap();
    assert!(storage::load(&config).is_err());
}

#[test]
fn transparency_settings_preserve_projects_and_read_legacy_configs() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("projects.json");
    fs::write(&path, r#"{"paths":["/example/project"]}"#).unwrap();
    let mut config = storage::load_config(&path).unwrap();
    assert_eq!(config.transparency, 0);
    assert!(!config.keep_text_opaque);
    assert!(config.show_stats);
    let original = config.paths.clone();
    config.transparency = 25;
    config.keep_text_opaque = true;
    config.show_stats = false;
    storage::save_config(&path, &config).unwrap();
    let restored = storage::load_config(&path).unwrap();
    assert_eq!(restored.transparency, 25);
    assert!(restored.keep_text_opaque);
    assert!(!restored.show_stats);
    assert_eq!(restored.paths, original);

    storage::save(&path, vec![]).unwrap();
    let restored = storage::load_config(&path).unwrap();
    assert!(restored.paths.is_empty());
    assert_eq!(restored.transparency, 25);
    assert!(restored.keep_text_opaque);
    assert!(!restored.show_stats);

    config.transparency = 51;
    assert!(storage::save_config(&path, &config).is_err());
    assert_eq!(storage::load_config(&path).unwrap().transparency, 25);
    let invalid = r#"{"paths":[],"transparency":99}"#;
    fs::write(&path, invalid).unwrap();
    assert!(storage::load_config(&path).is_err());
    assert!(storage::save(&path, vec![]).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), invalid);
}

#[tokio::test]
async fn merge_conflicts_block_actions_and_preserve_files() {
    let f = Fixture::new();
    fs::write(f.a.join("initial.txt"), "local version").unwrap();
    run(&f.a, &["add", "--all"]);
    run(&f.a, &["commit", "-m", "Local version"]);
    fs::write(f.b.join("initial.txt"), "remote version").unwrap();
    run(&f.b, &["add", "--all"]);
    run(&f.b, &["commit", "-m", "Remote version"]);
    run(&f.b, &["push"]);
    run(&f.a, &["fetch"]);
    let output = Command::new("git")
        .arg("-C")
        .arg(&f.a)
        .args(["merge", "origin/main"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let before = fs::read(f.a.join("initial.txt")).unwrap();
    let s = git::inspect(&f.a, false).await.unwrap();
    assert!(s.conflicts);
    assert!(!s.can_pull() && !s.can_push());
    assert!(
        git::perform(f.a.clone(), Action::Commit("Should fail".into()))
            .await
            .is_err()
    );
    assert_eq!(fs::read(f.a.join("initial.txt")).unwrap(), before);
}
