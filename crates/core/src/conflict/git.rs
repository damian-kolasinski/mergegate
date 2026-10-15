use crate::git::{GitError, Upstream};
use crate::project::ProjectName;

/// What the conflict watch needs from a project's staging repo.
pub trait ConflictGit {
    /// The paths merging `theirs` into `ours` conflicts on, none when it's
    /// clean. Writes no commit and moves no ref.
    fn conflicts(
        &self,
        project: &ProjectName,
        ours: &str,
        theirs: &str,
    ) -> Result<Vec<String>, GitError>;

    /// Fetches `upstream`'s `target` into a ref only the watch writes, and
    /// returns its commit. Staging's own `target` stays as it is, so this
    /// runs without the store, alongside a submit or merge fetching it.
    fn fetch_watched(
        &self,
        project: &ProjectName,
        upstream: &Upstream,
        target: &str,
    ) -> Result<String, GitError>;

    /// Moves staging's `target` to `commit` when that fast-forwards it. One
    /// that went elsewhere since, as a submit's fetch may take it, stays.
    fn advance(&self, project: &ProjectName, target: &str, commit: &str) -> Result<(), GitError>;
}
