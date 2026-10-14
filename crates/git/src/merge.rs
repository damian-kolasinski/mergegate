//! The local backend's merge: squash the item onto its target in staging,
//! then push that one commit to the operator's repo.

use std::ffi::{OsStr, OsString};
use std::path::Path;

use mergegate_core::git::{GitError, Upstream};
use mergegate_core::item::Item;
use mergegate_core::merge::{Landing, MergeBackend};
use mergegate_core::project::{Backend, Project};

use crate::command::{self, failed, output_of, run_command};
use crate::trial::Merged;
use crate::{HubGit, staging};

impl MergeBackend for HubGit {
    fn kind(&self) -> Backend {
        Backend::Local
    }

    fn land(&self, project: &Project, item: &Item, message: &str) -> Result<Landing, GitError> {
        let operator = Identity::of(&project.upstream)?;
        // A commit landing on the target between the fetch and the push
        // turns the push down; one more go takes it in.
        for _ in 0..2 {
            if let Some(landing) = self.attempt(project, item, message, &operator)? {
                return Ok(landing);
            }
        }
        Ok(Landing::Held {
            reason: format!(
                "{} in {} moved twice while merging",
                item.target,
                project.upstream.display()
            ),
        })
    }
}

impl HubGit {
    /// None when the target moved under the push.
    fn attempt(
        &self,
        project: &Project,
        item: &Item,
        message: &str,
        operator: &Identity,
    ) -> Result<Option<Landing>, GitError> {
        let dir = self.staging(&project.name);
        let git_dir = self.git_dir(&project.name);
        let target = &item.target;
        let upstream = Upstream::Repo(project.upstream.clone());
        staging::fetch(&self.home, &dir, &upstream, target)?;
        let branch = format!("refs/heads/{target}");
        let target_commit = self.rev_parse(&project.name, &format!("{branch}^{{commit}}"))?;
        let tree = match self.merge_tree(&project.name, &target_commit, &item.revision.head)? {
            Merged::Tree(tree) => tree,
            Merged::Conflict(paths) => {
                return Ok(Some(Landing::Conflict {
                    target_commit,
                    paths,
                }));
            }
        };
        let target_tree = self.rev_parse(&project.name, &format!("{target_commit}^{{tree}}"))?;
        if tree == target_tree {
            // Nothing left to land: the item's changes are on the target
            // already, as after a merge whose push the hub never recorded.
            return Ok(Some(Landing::Merged {
                commit: target_commit,
            }));
        }

        let mut commit_tree = command::git(&self.home);
        commit_tree
            .args([
                git_dir.as_os_str(),
                arg("commit-tree"),
                arg(&tree),
                arg("-p"),
                arg(&target_commit),
                arg("-m"),
                arg(message),
            ])
            .envs(operator.env());
        let commit = run_command(commit_tree, "writing the merge commit")?
            .trim()
            .to_owned();

        match push(&dir, &project.upstream, &commit, &branch)? {
            Pushed::Done => {}
            Pushed::Moved => return Ok(None),
            Pushed::Refused(reason) => {
                return Ok(Some(Landing::Held {
                    reason: format!("{} refused {target}: {reason}", project.upstream.display()),
                }));
            }
        }
        // So staging's target is the merge, for the next item's diff. The
        // commit has landed, so a failing fetch mustn't report a hold; every
        // submit and merge fetches the target again before reading it.
        let _ = staging::fetch(&self.home, &dir, &upstream, target);
        Ok(Some(Landing::Merged { commit }))
    }
}

enum Pushed {
    Done,
    /// Not a fast-forward any more.
    Moved,
    /// The operator's repo turned it down, as it does while the target is
    /// checked out with changes; the reason is its own.
    Refused(String),
}

/// Pushes as the operator, so their hooks run. Never forced. A checked-out
/// target updates in place when its checkout is clean, as if they had pulled.
fn push(staging: &Path, upstream: &Path, commit: &str, branch: &str) -> Result<Pushed, GitError> {
    let mut git_dir = OsString::from("--git-dir=");
    git_dir.push(staging);
    let refspec = format!("{commit}:{branch}");
    let mut cmd = command::operator();
    cmd.args([
        git_dir.as_os_str(),
        arg("push"),
        arg("--porcelain"),
        arg("--receive-pack=git -c receive.denyCurrentBranch=updateInstead receive-pack"),
        upstream.as_os_str(),
        arg(&refspec),
    ]);
    let doing = format!("pushing the merge to {}", upstream.display());
    let output = output_of(cmd, &doing)?;
    if output.status.success() {
        return Ok(Pushed::Done);
    }
    // `!<TAB>from:to<TAB>[rejected] (why)` for each ref turned down.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let Some(summary) = stdout
        .lines()
        .find_map(|line| line.strip_prefix("!\t"))
        .and_then(|line| line.split('\t').nth(1))
    else {
        return Err(failed(&doing, &output));
    };
    let why = summary
        .split_once(" (")
        .map_or(summary, |(_, why)| why.trim_end_matches(')'));
    if summary.starts_with("[rejected]") && matches!(why, "non-fast-forward" | "fetch first") {
        return Ok(Pushed::Moved);
    }
    Ok(Pushed::Refused(why.to_owned()))
}

/// Who the operator's repo says commits there: the merge commit is theirs,
/// as if they had squashed it themselves.
struct Identity {
    author: Ident,
    committer: Ident,
}

struct Ident {
    name: String,
    email: String,
    /// `<seconds> <offset>`, which git's date variables take as is.
    date: String,
}

impl Identity {
    fn of(repo: &Path) -> Result<Self, GitError> {
        let read = |var| {
            let mut cmd = command::operator();
            cmd.current_dir(repo).args(["var", var]);
            let ident = run_command(cmd, &format!("reading {var} in {}", repo.display()))?;
            Ident::parse(ident.trim())
                .ok_or_else(|| GitError::Failed(format!("{var} {ident:?} is not an identity")))
        };
        Ok(Self {
            author: read("GIT_AUTHOR_IDENT")?,
            committer: read("GIT_COMMITTER_IDENT")?,
        })
    }

    fn env(&self) -> [(&'static str, &str); 6] {
        [
            ("GIT_AUTHOR_NAME", &self.author.name),
            ("GIT_AUTHOR_EMAIL", &self.author.email),
            ("GIT_AUTHOR_DATE", &self.author.date),
            ("GIT_COMMITTER_NAME", &self.committer.name),
            ("GIT_COMMITTER_EMAIL", &self.committer.email),
            ("GIT_COMMITTER_DATE", &self.committer.date),
        ]
    }
}

impl Ident {
    /// `Name <email> 1700000000 +0200`, as `git var` prints it.
    fn parse(ident: &str) -> Option<Self> {
        let (name, rest) = ident.split_once(" <")?;
        let (email, date) = rest.rsplit_once("> ")?;
        Some(Self {
            name: name.to_owned(),
            email: email.to_owned(),
            date: date.to_owned(),
        })
    }
}

fn arg(s: &str) -> &OsStr {
    OsStr::new(s)
}

#[cfg(test)]
#[path = "merge_tests.rs"]
mod tests;
