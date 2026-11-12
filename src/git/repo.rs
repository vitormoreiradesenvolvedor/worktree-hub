use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::cmd;

#[derive(Clone, Debug)]
pub struct RepoInfo {
    /// Top-level working directory of the repository (or the git dir, when bare).
    pub root: PathBuf,
    pub name: String,
    pub bare: bool,
}

pub fn discover(path: &Path) -> Result<RepoInfo> {
    let dir = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent().unwrap_or(path).to_path_buf()
    };

    let bare = cmd::git(&dir, ["rev-parse", "--is-bare-repository"])
        .map(|s| s.trim() == "true")
        .unwrap_or(false);

    let root = if bare {
        let common = cmd::git(
            &dir,
            ["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )
        .context("não parece ser um repositório git")?;
        PathBuf::from(common.trim())
    } else {
        let top = cmd::git(&dir, ["rev-parse", "--show-toplevel"])
            .context("não parece ser um repositório git")?;
        PathBuf::from(top.trim())
    };

    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().trim_end_matches(".git").to_owned())
        .unwrap_or_else(|| root.display().to_string());

    Ok(RepoInfo { root, name, bare })
}

/// Working-tree state of a single worktree.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    pub branch: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub staged: u32,
    pub unstaged: u32,
    pub untracked: u32,
    pub conflicts: u32,
}

impl Status {
    pub fn dirty(&self) -> bool {
        self.staged + self.unstaged + self.untracked + self.conflicts > 0
    }

    pub fn changes(&self) -> u32 {
        self.staged + self.unstaged + self.untracked + self.conflicts
    }
}

/// Parses `git status --porcelain=v2 --branch --untracked-files=normal`.
pub fn parse_status(text: &str) -> Status {
    let mut st = Status::default();

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("# branch.head ") {
            let head = rest.trim();
            st.branch = if head == "(detached)" {
                None
            } else {
                Some(head.to_owned())
            };
        } else if let Some(rest) = line.strip_prefix("# branch.upstream ") {
            st.upstream = Some(rest.trim().to_owned());
        } else if let Some(rest) = line.strip_prefix("# branch.ab ") {
            for token in rest.split_whitespace() {
                match token.split_at(1) {
                    ("+", n) => st.ahead = n.parse().unwrap_or(0),
                    ("-", n) => st.behind = n.parse().unwrap_or(0),
                    _ => {}
                }
            }
        } else if let Some(rest) = line.strip_prefix("1 ").or_else(|| line.strip_prefix("2 ")) {
            // "<XY> <sub> ..." — X is the staged change, Y the unstaged one.
            let xy = rest.as_bytes();
            if xy.len() >= 2 {
                if xy[0] != b'.' {
                    st.staged += 1;
                }
                if xy[1] != b'.' {
                    st.unstaged += 1;
                }
            }
        } else if line.starts_with("u ") {
            st.conflicts += 1;
        } else if line.starts_with("? ") {
            st.untracked += 1;
        }
    }

    st
}

pub fn status(worktree: &Path) -> Result<Status> {
    let text = cmd::git(
        worktree,
        [
            "status",
            "--porcelain=v2",
            "--branch",
            "--untracked-files=normal",
        ],
    )?;
    Ok(parse_status(&text))
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Commit {
    pub short: String,
    pub subject: String,
    pub author: String,
    pub when: String,
}

pub fn last_commit(worktree: &Path) -> Result<Option<Commit>> {
    let out = cmd::raw(worktree, ["log", "-1", "--format=%h%x1f%s%x1f%an%x1f%ar"])?;
    if !out.ok {
        // A brand new worktree with --no-checkout, or an empty repo: no commits yet.
        return Ok(None);
    }
    let line = out.stdout.trim_end_matches('\n').to_owned();
    if line.trim().is_empty() {
        return Ok(None);
    }
    let mut parts = line.split('\u{1f}');
    Ok(Some(Commit {
        short: parts.next().unwrap_or_default().to_owned(),
        subject: parts.next().unwrap_or_default().to_owned(),
        author: parts.next().unwrap_or_default().to_owned(),
        when: parts.next().unwrap_or_default().to_owned(),
    }))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BranchRef {
    pub name: String,
    pub remote: bool,
}

/// Local branches first (most recently committed first), then remote ones.
pub fn branches(repo: &Path) -> Result<Vec<BranchRef>> {
    let text = cmd::git(
        repo,
        [
            "for-each-ref",
            "--sort=-committerdate",
            "--format=%(refname:short)%1f%(refname)",
            "refs/heads",
            "refs/remotes",
        ],
    )?;

    let mut out = Vec::new();
    for line in text.lines() {
        let mut parts = line.split('\u{1f}');
        let short = parts.next().unwrap_or_default().trim();
        let full = parts.next().unwrap_or_default().trim();
        if short.is_empty() || short.ends_with("/HEAD") {
            continue;
        }
        out.push(BranchRef {
            name: short.to_owned(),
            remote: full.starts_with("refs/remotes/"),
        });
    }
    Ok(out)
}

/// Best guess for the branch a new worktree should start from.
pub fn default_base(repo: &Path, known: &[BranchRef]) -> String {
    if let Ok(text) = cmd::git(
        repo,
        ["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    ) {
        let name = text.trim();
        if !name.is_empty() {
            return name.to_owned();
        }
    }
    for candidate in ["main", "master", "develop", "development"] {
        if known.iter().any(|b| !b.remote && b.name == candidate) {
            return candidate.to_owned();
        }
    }
    known
        .iter()
        .find(|b| !b.remote)
        .map(|b| b.name.clone())
        .unwrap_or_else(|| "HEAD".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_status_is_not_dirty() {
        let st = parse_status("# branch.oid abc\n# branch.head main\n");
        assert_eq!(st.branch.as_deref(), Some("main"));
        assert!(!st.dirty());
        assert_eq!(st.changes(), 0);
    }

    #[test]
    fn counts_staged_unstaged_untracked_and_conflicts() {
        let text = "\
# branch.oid abc
# branch.head feature/x
# branch.upstream origin/feature/x
# branch.ab +3 -1
1 M. N... 100644 100644 100644 aaa bbb src/a.rs
1 .M N... 100644 100644 100644 aaa bbb src/b.rs
1 MM N... 100644 100644 100644 aaa bbb src/c.rs
2 R. N... 100644 100644 100644 aaa bbb R100 new\told
u UU N... 100644 100644 100644 100644 aaa bbb ccc src/conflict.rs
? notes.txt
! ignored.txt
";
        let st = parse_status(text);
        assert_eq!(st.ahead, 3);
        assert_eq!(st.behind, 1);
        assert_eq!(st.upstream.as_deref(), Some("origin/feature/x"));
        assert_eq!(st.staged, 3, "M., MM and R. are staged");
        assert_eq!(st.unstaged, 2, ".M and MM are unstaged");
        assert_eq!(st.untracked, 1);
        assert_eq!(st.conflicts, 1);
        assert!(st.dirty());
        assert_eq!(st.changes(), 7);
    }

    #[test]
    fn detached_head_has_no_branch() {
        let st = parse_status("# branch.head (detached)\n");
        assert!(st.branch.is_none());
    }

    #[test]
    fn default_base_prefers_known_main() {
        let branches = vec![
            BranchRef {
                name: "feature/z".into(),
                remote: false,
            },
            BranchRef {
                name: "main".into(),
                remote: false,
            },
        ];
        // No repo on disk, so the origin/HEAD lookup fails and we fall back.
        let base = default_base(Path::new("/nonexistent-repo-path"), &branches);
        assert_eq!(base, "main");
    }
}
