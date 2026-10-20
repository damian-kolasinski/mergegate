//! Folder listings module detection reads, through gix.

use std::collections::HashMap;

use mergegate_core::git::GitError;
use mergegate_core::module::{Entry, ModuleGit};
use mergegate_core::project::ProjectName;

use crate::HubGit;

/// A folder's entries: name, whether it's a folder, and its object.
type Listing = Vec<(String, bool, gix::ObjectId)>;

impl ModuleGit for HubGit {
    fn listings(
        &self,
        project: &ProjectName,
        commit: &str,
        dirs: &[&str],
    ) -> Result<Vec<Vec<Entry>>, GitError> {
        let dir = self.staging(project);
        let failed = |e: &dyn std::fmt::Display| {
            GitError::Failed(format!(
                "listing folders of {commit} in {} failed: {e}",
                dir.display()
            ))
        };
        let repo = gix::open_opts(&dir, gix::open::Options::isolated()).map_err(|e| failed(&e))?;
        let id = gix::ObjectId::from_hex(commit.as_bytes()).map_err(|e| failed(&e))?;
        let root = repo
            .find_commit(id)
            .map_err(|e| failed(&e))?
            .tree_id()
            .map_err(|e| failed(&e))?
            .detach();
        let mut listed = HashMap::from([(
            String::new(),
            Some(list(&repo, root).map_err(|e| failed(&e))?),
        )]);
        dirs.iter()
            .map(|path| {
                fill(&repo, &mut listed, path).map_err(|e| failed(&e))?;
                Ok(listed[*path]
                    .iter()
                    .flatten()
                    .map(|(name, dir, _)| Entry {
                        name: name.clone(),
                        dir: *dir,
                    })
                    .collect())
            })
            .collect()
    }
}

/// Lists `path` into `listed` from its parent's listing, listing the parent
/// first if it isn't yet: each folder on the way is read once, rather than
/// once for every folder under it. None when `path` isn't a folder there.
fn fill(
    repo: &gix::Repository,
    listed: &mut HashMap<String, Option<Listing>>,
    path: &str,
) -> Result<(), String> {
    if listed.contains_key(path) {
        return Ok(());
    }
    let (parent, name) = path.rsplit_once('/').unwrap_or(("", path));
    fill(repo, listed, parent)?;
    let tree = listed[parent]
        .iter()
        .flatten()
        .find(|(entry, dir, _)| *dir && entry == name)
        .map(|(_, _, id)| *id);
    let listing = tree.map(|id| list(repo, id)).transpose()?;
    listed.insert(path.to_owned(), listing);
    Ok(())
}

fn list(repo: &gix::Repository, id: gix::ObjectId) -> Result<Listing, String> {
    let tree = repo
        .find_object(id)
        .map_err(|e| e.to_string())?
        .try_into_tree()
        .map_err(|e| e.to_string())?;
    let decoded = tree.decode().map_err(|e| e.to_string())?;
    Ok(decoded
        .entries
        .iter()
        .map(|e| (e.filename.to_string(), e.mode.is_tree(), e.oid.to_owned()))
        .collect())
}
