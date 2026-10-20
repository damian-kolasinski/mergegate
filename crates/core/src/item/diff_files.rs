//! Diffing a revision's files at submit, so opening it later only reads.

use super::{ChangedFile, FileStat, ItemError};
use crate::diff::{DiffGit, DiffStore, FileDiff, compute};
use crate::git::GitError;
use crate::project::ProjectName;
use crate::triage::Triage;

/// Files whose blobs are read at once: few enough git reads, and few enough
/// blobs held in memory together.
const BATCH: usize = 64;

/// Counts each file's lines, leaving it untriaged, diffing and caching the blob pairs the cache
/// doesn't have yet.
pub(crate) fn diff_files(
    store: &mut impl DiffStore,
    git: &impl DiffGit,
    project: &ProjectName,
    files: Vec<ChangedFile>,
) -> Result<Vec<FileStat>, ItemError> {
    let mut counts = Vec::with_capacity(files.len());
    let mut uncached = Vec::new();
    for (index, file) in files.iter().enumerate() {
        match store.diff(file.old_blob.as_deref(), file.new_blob.as_deref())? {
            Some(diff) => counts.push((diff.additions, diff.deletions)),
            None => {
                counts.push((None, None));
                uncached.push(index);
            }
        }
    }
    let missing: Vec<&ChangedFile> = uncached.iter().map(|&i| &files[i]).collect();
    let diffs = compute_diffs(git, project, &missing)?;
    for (&index, diff) in uncached.iter().zip(&diffs) {
        let file = &files[index];
        store.insert_diff(file.old_blob.as_deref(), file.new_blob.as_deref(), diff)?;
        counts[index] = (diff.additions, diff.deletions);
    }
    Ok(files
        .into_iter()
        .zip(counts)
        .map(|(file, (additions, deletions))| FileStat {
            file,
            additions,
            deletions,
            generated: false,
            triage: Triage::default(),
            placement: None,
        })
        .collect())
}

/// Diffs `files` from their blobs in staging, in order, caching nothing.
pub(super) fn compute_diffs(
    git: &impl DiffGit,
    project: &ProjectName,
    files: &[&ChangedFile],
) -> Result<Vec<FileDiff>, GitError> {
    let mut diffs = Vec::with_capacity(files.len());
    for batch in files.chunks(BATCH) {
        let ids: Vec<&str> = batch
            .iter()
            .flat_map(|f| [f.old_blob.as_deref(), f.new_blob.as_deref()])
            .flatten()
            .collect();
        let mut blobs = git.blobs(project, &ids)?.into_iter();
        // A side without an ID is an empty file; one whose ID names no blob
        // can't be shown as text.
        let mut side = |id: Option<&str>| match id {
            Some(_) => blobs.next().flatten(),
            None => Some(Vec::new()),
        };
        for file in batch {
            let (old, new) = (file.old_blob.as_deref(), file.new_blob.as_deref());
            diffs.push(match (side(old), side(new)) {
                (Some(old), Some(new)) => compute(&old, &new),
                _ => FileDiff::binary(),
            });
        }
    }
    Ok(diffs)
}
