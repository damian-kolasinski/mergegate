use super::DiffGit;
use crate::project::{ProjectError, ProjectStore, find};

/// The blob `id` in `project`'s staging repo, for showing a file in full;
/// `None` when no blob has that ID.
pub fn blob(
    store: &impl ProjectStore,
    git: &impl DiffGit,
    project: &str,
    id: &str,
) -> Result<Option<Vec<u8>>, ProjectError> {
    let project = find(store, project)?;
    Ok(git.blobs(&project.name, &[id])?.pop().flatten())
}

#[cfg(test)]
#[path = "blob_tests.rs"]
mod tests;
