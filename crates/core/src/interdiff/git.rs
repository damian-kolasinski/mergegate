use crate::git::GitError;
use crate::project::ProjectName;

/// What interdiffs need from the project's staging repo, besides what items
/// do.
pub trait InterdiffGit {
    /// Replays what `head` changes since `base` onto `onto`, a three-way
    /// merge written as a tree to staging and returned without a commit.
    fn replay(
        &self,
        project: &ProjectName,
        base: &str,
        onto: &str,
        head: &str,
    ) -> Result<Replay, GitError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replay {
    pub tree: String,
    /// Paths both sides changed apart, in path order; the tree holds them
    /// with conflict markers.
    pub conflicts: Vec<String>,
}
