//! Blob reads for diffing, through gix.

use mergegate_core::diff::DiffGit;
use mergegate_core::git::GitError;
use mergegate_core::project::ProjectName;

use crate::HubGit;

impl DiffGit for HubGit {
    fn blobs(&self, project: &ProjectName, ids: &[&str]) -> Result<Vec<Option<Vec<u8>>>, GitError> {
        let dir = self.staging(project);
        let failed = |e: &dyn std::fmt::Display| {
            GitError::Failed(format!("reading blobs in {} failed: {e}", dir.display()))
        };
        let repo = gix::open_opts(&dir, gix::open::Options::isolated()).map_err(|e| failed(&e))?;
        ids.iter()
            .map(|id| {
                let Ok(id) = gix::ObjectId::from_hex(id.as_bytes()) else {
                    return Ok(None);
                };
                let object = repo.try_find_object(id).map_err(|e| failed(&e))?;
                Ok(object
                    .filter(|o| o.kind == gix::object::Kind::Blob)
                    .map(|o| o.detach().data))
            })
            .collect()
    }
}
