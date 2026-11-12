use std::path::{Path, PathBuf};

use anyhow::Result;

use super::cmd;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    pub head: Option<String>,
    /// Short branch name (`refs/heads/x` is stripped to `x`).
    pub branch: Option<String>,
    pub bare: bool,
    pub detached: bool,
    pub locked: Option<String>,
    pub prunable: Option<String>,
    /// The first entry reported by git is the main worktree; it cannot be removed.
    pub is_main: bool,
}

impl Worktree {
    pub fn name(&self) -> String {
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.display().to_string())
    }

    pub fn short_head(&self) -> String {
        self.head
            .as_deref()
            .map(|h| h.chars().take(7).collect())
            .unwrap_or_else(|| "-".to_owned())
    }

    pub fn ref_label(&self) -> String {
        match (&self.branch, self.detached) {
            (Some(b), _) => b.clone(),
            (None, true) => format!("detached @ {}", self.short_head()),
            (None, false) => self.short_head(),
        }
    }

    pub fn exists(&self) -> bool {
        self.path.is_dir()
    }
}

/// Parses the output of `git worktree list --porcelain`.
pub fn parse_porcelain(text: &str) -> Vec<Worktree> {
    let mut out: Vec<Worktree> = Vec::new();
    let mut current: Option<Worktree> = None;

    for line in text.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            if let Some(wt) = current.take() {
                out.push(wt);
            }
            continue;
        }

        let (key, value) = match line.split_once(' ') {
            Some((k, v)) => (k, v.trim()),
            None => (line, ""),
        };

        match key {
            "worktree" => {
                if let Some(wt) = current.take() {
                    out.push(wt);
                }
                current = Some(Worktree {
                    path: PathBuf::from(value),
                    is_main: out.is_empty(),
                    ..Default::default()
                });
            }
            "HEAD" => {
                if let Some(wt) = current.as_mut() {
                    wt.head = Some(value.to_owned());
                }
            }
            "branch" => {
                if let Some(wt) = current.as_mut() {
                    wt.branch = Some(value.trim_start_matches("refs/heads/").to_owned());
                }
            }
            "bare" => {
                if let Some(wt) = current.as_mut() {
                    wt.bare = true;
                }
            }
            "detached" => {
                if let Some(wt) = current.as_mut() {
                    wt.detached = true;
                }
            }
            "locked" => {
                if let Some(wt) = current.as_mut() {
                    wt.locked = Some(if value.is_empty() {
                        "sem motivo informado".to_owned()
                    } else {
                        value.to_owned()
                    });
                }
            }
            "prunable" => {
                if let Some(wt) = current.as_mut() {
                    wt.prunable = Some(if value.is_empty() {
                        "pode ser removido".to_owned()
                    } else {
                        value.to_owned()
                    });
                }
            }
            _ => {}
        }
    }

    if let Some(wt) = current.take() {
        out.push(wt);
    }
    out
}

pub fn list(repo: &Path) -> Result<Vec<Worktree>> {
    let text = cmd::git(repo, ["worktree", "list", "--porcelain"])?;
    Ok(parse_porcelain(&text))
}

#[derive(Clone, Debug, Default)]
pub struct AddOptions {
    /// Directory the new worktree lives in.
    pub path: PathBuf,
    /// Branch to create (when `create_branch`) or to check out.
    pub branch: String,
    pub create_branch: bool,
    /// Start point for a new branch (`main`, `origin/main`, a sha…).
    pub base: String,
    pub detach: bool,
    pub force: bool,
    /// `git worktree add --no-checkout`
    pub no_checkout: bool,
    /// Overwrite an existing branch of the same name (`-B` instead of `-b`).
    pub force_branch: bool,
    pub track: bool,
}

impl AddOptions {
    /// The exact argument list handed to git, also used to show the command in the UI.
    pub fn args(&self) -> Vec<String> {
        let mut args: Vec<String> = vec!["worktree".into(), "add".into()];
        if self.force {
            args.push("--force".into());
        }
        if self.no_checkout {
            args.push("--no-checkout".into());
        }
        if self.detach {
            args.push("--detach".into());
        } else if self.create_branch && !self.branch.trim().is_empty() {
            args.push(if self.force_branch {
                "-B".into()
            } else {
                "-b".into()
            });
            args.push(self.branch.trim().to_owned());
        }
        if self.track && !self.detach {
            args.push("--track".into());
        }
        args.push(self.path.to_string_lossy().into_owned());

        // Commit-ish: the start point for a new branch, or the branch/ref to check out.
        let commitish = if self.create_branch || self.detach {
            self.base.trim()
        } else {
            self.branch.trim()
        };
        if !commitish.is_empty() {
            args.push(commitish.to_owned());
        }
        args
    }

    pub fn preview(&self) -> String {
        format!("git {}", self.args().join(" "))
    }
}

pub fn add(repo: &Path, opts: &AddOptions) -> Result<String> {
    cmd::git(repo, opts.args())
}

pub fn remove(repo: &Path, path: &Path, force: bool) -> Result<String> {
    let mut args: Vec<String> = vec!["worktree".into(), "remove".into()];
    if force {
        args.push("--force".into());
    }
    args.push(path.to_string_lossy().into_owned());
    cmd::git(repo, args)
}

pub fn prune(repo: &Path) -> Result<String> {
    cmd::git(repo, ["worktree", "prune", "-v"])
}

pub fn lock(repo: &Path, path: &Path, reason: &str) -> Result<String> {
    let mut args: Vec<String> = vec!["worktree".into(), "lock".into()];
    if !reason.trim().is_empty() {
        args.push("--reason".into());
        args.push(reason.trim().to_owned());
    }
    args.push(path.to_string_lossy().into_owned());
    cmd::git(repo, args)
}

pub fn unlock(repo: &Path, path: &Path) -> Result<String> {
    cmd::git(
        repo,
        [
            "worktree".to_owned(),
            "unlock".to_owned(),
            path.to_string_lossy().into_owned(),
        ],
    )
}

pub fn delete_branch(repo: &Path, branch: &str, force: bool) -> Result<String> {
    cmd::git(
        repo,
        [
            "branch".to_owned(),
            if force {
                "-D".to_owned()
            } else {
                "-d".to_owned()
            },
            branch.to_owned(),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
worktree /home/u/proj
HEAD 0a1b2c3d4e5f60718293a4b5c6d7e8f901234567
branch refs/heads/main

worktree /home/u/proj-worktrees/feature-x
HEAD 1111111111111111111111111111111111111111
branch refs/heads/feature/x

worktree /home/u/proj-worktrees/spike
HEAD 2222222222222222222222222222222222222222
detached
locked investigando bug

worktree /home/u/gone
HEAD 3333333333333333333333333333333333333333
branch refs/heads/old
prunable gitdir file points to non-existent location
";

    #[test]
    fn parses_every_entry() {
        let list = parse_porcelain(SAMPLE);
        assert_eq!(list.len(), 4);
    }

    #[test]
    fn first_entry_is_main_and_keeps_branch() {
        let list = parse_porcelain(SAMPLE);
        assert!(list[0].is_main);
        assert_eq!(list[0].branch.as_deref(), Some("main"));
        assert_eq!(list[0].short_head(), "0a1b2c3");
        assert!(!list[1].is_main);
    }

    #[test]
    fn strips_refs_heads_prefix_from_nested_branch() {
        let list = parse_porcelain(SAMPLE);
        assert_eq!(list[1].branch.as_deref(), Some("feature/x"));
        assert_eq!(list[1].name(), "feature-x");
    }

    #[test]
    fn reads_detached_locked_and_prunable() {
        let list = parse_porcelain(SAMPLE);
        assert!(list[2].detached);
        assert_eq!(list[2].locked.as_deref(), Some("investigando bug"));
        assert!(list[2].ref_label().starts_with("detached @"));
        assert!(list[3].prunable.is_some());
    }

    #[test]
    fn handles_bare_main_worktree_without_trailing_blank_line() {
        let list = parse_porcelain("worktree /srv/repo.git\nbare");
        assert_eq!(list.len(), 1);
        assert!(list[0].bare);
        assert!(list[0].is_main);
    }

    #[test]
    fn add_args_create_branch_from_base() {
        let opts = AddOptions {
            path: PathBuf::from("/tmp/wt/feat"),
            branch: "feature/login".into(),
            create_branch: true,
            base: "origin/main".into(),
            ..Default::default()
        };
        assert_eq!(
            opts.args(),
            vec![
                "worktree",
                "add",
                "-b",
                "feature/login",
                "/tmp/wt/feat",
                "origin/main"
            ]
        );
    }

    #[test]
    fn add_args_checkout_existing_branch() {
        let opts = AddOptions {
            path: PathBuf::from("/tmp/wt/hot"),
            branch: "hotfix".into(),
            create_branch: false,
            ..Default::default()
        };
        assert_eq!(
            opts.args(),
            vec!["worktree", "add", "/tmp/wt/hot", "hotfix"]
        );
    }

    #[test]
    fn add_args_detach_ignores_branch_flag() {
        let opts = AddOptions {
            path: PathBuf::from("/tmp/wt/d"),
            branch: "whatever".into(),
            create_branch: true,
            detach: true,
            base: "v1.2.3".into(),
            force: true,
            ..Default::default()
        };
        assert_eq!(
            opts.args(),
            vec![
                "worktree",
                "add",
                "--force",
                "--detach",
                "/tmp/wt/d",
                "v1.2.3"
            ]
        );
    }

    #[test]
    fn add_args_force_branch_uses_capital_b() {
        let opts = AddOptions {
            path: PathBuf::from("/tmp/wt/f"),
            branch: "dup".into(),
            create_branch: true,
            force_branch: true,
            base: "main".into(),
            ..Default::default()
        };
        assert!(opts.args().contains(&"-B".to_owned()));
        assert!(opts.preview().starts_with("git worktree add -B dup"));
    }
}
