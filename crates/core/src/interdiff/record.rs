//! Interdiffs computed when a revision is submitted, so opening one only
//! reads.

use std::collections::BTreeMap;

use super::{Change, InterdiffFile, InterdiffGit, NewInterdiff};
use crate::diff::{DiffGit, DiffStore};
use crate::item::{ChangedFile, FileStat, Item, ItemError, ItemGit, ItemStore, diff_files};
use crate::viewed::ViewedStore;

/// What a new revision records besides its own files.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Recorded {
    pub interdiffs: Vec<NewInterdiff>,
    /// The new revision's files that stay viewed, in path order.
    pub viewed: Vec<String>,
}

/// The interdiff from each of `item`'s revisions to the next one, which
/// changes `files` from `base` to `head`, with every diff it shows cached.
///
/// Over a moved base each earlier revision is replayed onto the new one, and
/// the replayed tree is pinned at `r<K>-r<N>` so the blobs the UI reads stay
/// in staging until the item closes. A file is carried viewed from the latest
/// revision it's unchanged since.
pub(crate) fn record<S: ItemStore + DiffStore + ViewedStore>(
    store: &mut S,
    git: &(impl ItemGit + InterdiffGit + DiffGit),
    item: &Item,
    head: &str,
    base: &str,
    files: &[FileStat],
) -> Result<Recorded, ItemError> {
    let project = item.id.project();
    let number = item.revision.number + 1;
    let mut interdiffs = Vec::new();
    for earlier in store.revisions(&item.id)? {
        let (from, conflicts) = if earlier.base == base {
            (earlier.head.clone(), Vec::new())
        } else {
            let replay = git.replay(project, &earlier.base, base, &earlier.head)?;
            let pin = format!("{}r{}-r{number}", item.pins(), earlier.number);
            git.pin(project, &pin, &replay.tree)?;
            (replay.tree, replay.conflicts)
        };
        let changed = git
            .changed_files(project, &from, head)?
            .into_iter()
            .flat_map(|file| unpaired(file, files))
            .filter(|file| !conflicts.contains(&file.path))
            .collect();
        let mut changed: BTreeMap<String, FileStat> = diff_files(store, git, project, changed)?
            .into_iter()
            .map(|stat| (stat.file.path.clone(), stat))
            .collect();
        let mut interdiff: Vec<InterdiffFile> = files
            .iter()
            .map(|own| {
                let path = &own.file.path;
                match changed.remove(path) {
                    _ if conflicts.contains(path) => InterdiffFile {
                        stat: own.clone(),
                        change: Change::Unseparated,
                    },
                    Some(stat) => InterdiffFile {
                        stat,
                        change: Change::Changed,
                    },
                    None => InterdiffFile {
                        stat: own.clone(),
                        change: Change::Unchanged,
                    },
                }
            })
            .collect();
        // A conflicted file the new revision doesn't change has nothing of
        // its own to show, so it was filtered out above, not dropped here.
        interdiff.extend(changed.into_values().map(|stat| InterdiffFile {
            stat,
            change: Change::Dropped,
        }));
        interdiff.sort_by(|a, b| a.stat.file.path.cmp(&b.stat.file.path));
        interdiffs.push(NewInterdiff {
            from: earlier.number,
            files: interdiff,
        });
    }
    let mut viewed = Vec::new();
    for own in files {
        let path = &own.file.path;
        let since = interdiffs.iter().rev().find(|interdiff| {
            interdiff
                .files
                .iter()
                .any(|f| f.stat.file.path == *path && f.change == Change::Unchanged)
        });
        if let Some(interdiff) = since
            && store.viewed(&item.id, interdiff.from)?.contains(path)
        {
            viewed.push(path.clone());
        }
    }
    Ok(Recorded { interdiffs, viewed })
}

/// `file`, split into a deletion and an addition when it renames away from
/// one of the new revision's own files. Files are matched by path, so that
/// own file would match nothing and read as unchanged since the earlier
/// revision. A rename from any other path, like one of a file the item added,
/// stays whole so it still reads as a move.
fn unpaired(file: ChangedFile, own: &[FileStat]) -> Vec<ChangedFile> {
    let Some(old_path) = file.old_path.clone() else {
        return vec![file];
    };
    if !own.iter().any(|o| o.file.path == old_path) {
        return vec![file];
    }
    vec![
        ChangedFile {
            path: old_path,
            old_path: None,
            old_blob: file.old_blob,
            new_blob: None,
        },
        ChangedFile {
            path: file.path,
            old_path: None,
            old_blob: None,
            new_blob: file.new_blob,
        },
    ]
}

#[cfg(test)]
#[path = "record_tests.rs"]
mod tests;
