use crate::git::GitError;
use crate::project::ProjectName;

/// What diffing needs from the project's staging repo.
pub trait DiffGit {
    /// The blobs with these IDs, in order: `None` for an ID that names no blob
    /// in staging, as a submodule's commit doesn't.
    fn blobs(&self, project: &ProjectName, ids: &[&str]) -> Result<Vec<Option<Vec<u8>>>, GitError>;
}
