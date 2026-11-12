//! Integration tests against a real `git` binary and real worktrees on disk.

use std::path::{Path, PathBuf};
use std::process::Command;

use worktree_hub::git::{self, AddOptions};

fn run(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "Teste")
        .env("GIT_AUTHOR_EMAIL", "teste@example.com")
        .env("GIT_COMMITTER_NAME", "Teste")
        .env("GIT_COMMITTER_EMAIL", "teste@example.com")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .expect("git deve estar instalado");
    assert!(
        out.status.success(),
        "git {args:?} falhou: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A repository with one commit on `main`.
fn repo() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    run(&repo, &["init", "-b", "main"]);
    std::fs::write(repo.join("README.md"), "# teste\n").unwrap();
    run(&repo, &["add", "."]);
    run(&repo, &["commit", "-m", "commit inicial"]);
    (tmp, repo)
}

fn add_opts(path: PathBuf, branch: &str) -> AddOptions {
    AddOptions {
        path,
        branch: branch.to_owned(),
        create_branch: true,
        base: "main".to_owned(),
        ..Default::default()
    }
}

#[test]
fn adds_lists_and_removes_a_worktree() {
    let (tmp, repo) = repo();
    let wt = tmp.path().join("wt-a");

    git::worktree::add(&repo, &add_opts(wt.clone(), "feature/a")).unwrap();

    let list = git::worktree::list(&repo).unwrap();
    assert_eq!(list.len(), 2, "main + nova worktree");
    assert!(list[0].is_main);
    let created = list.iter().find(|w| w.path == wt).expect("worktree criada");
    assert_eq!(created.branch.as_deref(), Some("feature/a"));
    assert!(created.exists());
    assert!(wt.join("README.md").is_file(), "checkout aconteceu");

    let status = git::repo::status(&wt).unwrap();
    assert!(!status.dirty());
    assert_eq!(status.branch.as_deref(), Some("feature/a"));

    let commit = git::repo::last_commit(&wt).unwrap().unwrap();
    assert_eq!(commit.subject, "commit inicial");

    git::worktree::remove(&repo, &wt, false).unwrap();
    assert_eq!(git::worktree::list(&repo).unwrap().len(), 1);
    assert!(!wt.exists());
}

#[test]
fn dirty_worktree_needs_force_to_be_removed() {
    let (tmp, repo) = repo();
    let wt = tmp.path().join("wt-dirty");
    git::worktree::add(&repo, &add_opts(wt.clone(), "feature/dirty")).unwrap();

    std::fs::write(wt.join("README.md"), "mudou\n").unwrap();
    std::fs::write(wt.join("novo.txt"), "novo\n").unwrap();

    let status = git::repo::status(&wt).unwrap();
    assert!(status.dirty());
    assert_eq!(status.unstaged, 1);
    assert_eq!(status.untracked, 1);

    assert!(
        git::worktree::remove(&repo, &wt, false).is_err(),
        "git deve recusar remover worktree com alterações"
    );
    git::worktree::remove(&repo, &wt, true).unwrap();
    assert_eq!(git::worktree::list(&repo).unwrap().len(), 1);
}

#[test]
fn prune_clears_the_record_of_a_deleted_folder() {
    let (tmp, repo) = repo();
    let wt = tmp.path().join("wt-gone");
    git::worktree::add(&repo, &add_opts(wt.clone(), "feature/gone")).unwrap();

    std::fs::remove_dir_all(&wt).unwrap();
    let list = git::worktree::list(&repo).unwrap();
    let orphan = list.iter().find(|w| w.path == wt).unwrap();
    assert!(orphan.prunable.is_some());
    assert!(!orphan.exists());

    git::worktree::prune(&repo).unwrap();
    assert_eq!(git::worktree::list(&repo).unwrap().len(), 1);
}

#[test]
fn locking_blocks_removal_until_unlocked() {
    let (tmp, repo) = repo();
    let wt = tmp.path().join("wt-locked");
    git::worktree::add(&repo, &add_opts(wt.clone(), "feature/locked")).unwrap();

    git::worktree::lock(&repo, &wt, "em uso pelo teste").unwrap();
    let list = git::worktree::list(&repo).unwrap();
    let locked = list.iter().find(|w| w.path == wt).unwrap();
    assert_eq!(locked.locked.as_deref(), Some("em uso pelo teste"));
    assert!(git::worktree::remove(&repo, &wt, false).is_err());

    git::worktree::unlock(&repo, &wt).unwrap();
    git::worktree::remove(&repo, &wt, false).unwrap();
}

#[test]
fn detached_worktree_reports_no_branch() {
    let (tmp, repo) = repo();
    let wt = tmp.path().join("wt-detached");
    let opts = AddOptions {
        path: wt.clone(),
        base: "main".to_owned(),
        detach: true,
        ..Default::default()
    };
    git::worktree::add(&repo, &opts).unwrap();

    let list = git::worktree::list(&repo).unwrap();
    let detached = list.iter().find(|w| w.path == wt).unwrap();
    assert!(detached.detached);
    assert!(detached.branch.is_none());
    assert!(detached.ref_label().starts_with("detached @"));
}

#[test]
fn checks_out_an_existing_branch_without_creating_it() {
    let (tmp, repo) = repo();
    run(&repo, &["branch", "hotfix"]);
    let wt = tmp.path().join("wt-hotfix");

    let opts = AddOptions {
        path: wt.clone(),
        branch: "hotfix".to_owned(),
        create_branch: false,
        ..Default::default()
    };
    git::worktree::add(&repo, &opts).unwrap();

    let list = git::worktree::list(&repo).unwrap();
    let created = list.iter().find(|w| w.path == wt).unwrap();
    assert_eq!(created.branch.as_deref(), Some("hotfix"));
}

#[test]
fn deletes_the_branch_after_removing_the_worktree() {
    let (tmp, repo) = repo();
    let wt = tmp.path().join("wt-branch");
    git::worktree::add(&repo, &add_opts(wt.clone(), "feature/bye")).unwrap();
    git::worktree::remove(&repo, &wt, false).unwrap();

    git::worktree::delete_branch(&repo, "feature/bye", true).unwrap();
    let names: Vec<String> = git::repo::branches(&repo)
        .unwrap()
        .into_iter()
        .map(|b| b.name)
        .collect();
    assert!(!names.contains(&"feature/bye".to_owned()), "{names:?}");
    assert!(names.contains(&"main".to_owned()));
}

#[test]
fn discovers_the_repository_from_a_subdirectory() {
    let (_tmp, repo) = repo();
    let nested = repo.join("src/deep");
    std::fs::create_dir_all(&nested).unwrap();

    let info = git::repo::discover(&nested).unwrap();
    assert_eq!(
        info.root.canonicalize().unwrap(),
        repo.canonicalize().unwrap()
    );
    assert_eq!(info.name, "repo");
    assert!(!info.bare);
}

#[test]
fn default_base_falls_back_to_the_repository_branch() {
    let (_tmp, repo) = repo();
    let branches = git::repo::branches(&repo).unwrap();
    assert_eq!(git::repo::default_base(&repo, &branches), "main");
}

#[test]
fn reports_ahead_and_behind_against_an_upstream() {
    let (tmp, repo) = repo();
    // A second clone acts as the remote so we get a real upstream.
    let clone = tmp.path().join("clone");
    run(
        tmp.path(),
        &["clone", repo.to_str().unwrap(), clone.to_str().unwrap()],
    );
    std::fs::write(clone.join("novo.txt"), "x\n").unwrap();
    run(&clone, &["add", "."]);
    run(&clone, &["commit", "-m", "um commit à frente"]);

    let status = git::repo::status(&clone).unwrap();
    assert_eq!(status.ahead, 1);
    assert_eq!(status.behind, 0);
    assert!(status.upstream.is_some());
    assert!(!status.dirty());
}
