use crate::git::GitError;
use crate::project::ProjectName;

/// What module detection reads from the project's staging repo.
pub trait ModuleGit {
    /// The entries of each of `dirs` in `commit`'s tree, in order; none for
    /// a directory the tree doesn't have.
    fn listings(
        &self,
        project: &ProjectName,
        commit: &str,
        dirs: &[&str],
    ) -> Result<Vec<Vec<Entry>>, GitError>;
}

/// A file or directory directly inside another.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Entry {
    pub name: String,
    pub dir: bool,
}
