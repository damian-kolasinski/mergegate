use crate::git::GitError;
use crate::project::ProjectName;

/// What triage reads from the project's staging repo.
pub trait TriageGit {
    /// Whether each of `paths` is `linguist-generated` by the
    /// `.gitattributes` files of `commit`'s tree, in order.
    fn generated(
        &self,
        project: &ProjectName,
        commit: &str,
        paths: &[&str],
    ) -> Result<Vec<bool>, GitError>;
}
