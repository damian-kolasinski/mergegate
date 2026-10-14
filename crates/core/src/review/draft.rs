//! Draft comments: written on lines of the diff, kept on the hub until the
//! operator submits a review, which sends them all (design §9.2).

use super::{
    Comment, CommentId, NewComment, QUOTE_LINES, ReviewError, ReviewStore, Side, reviewable,
};
use crate::diff::{Content, DiffGit, DiffStore};
use crate::item::{ItemId, ItemStore};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteDraft {
    pub id: ItemId,
    pub revision: u32,
    pub path: String,
    pub side: Side,
    pub start_line: u32,
    pub line: u32,
    pub body: String,
}

/// Drafts a comment on lines the revision's side of the file has. The comment
/// quotes them from the file, so it reads the same whatever the diff showed.
pub fn write_draft<S: ItemStore + DiffStore + ReviewStore>(
    store: &mut S,
    git: &impl DiffGit,
    request: WriteDraft,
) -> Result<Comment, ReviewError> {
    let WriteDraft {
        id,
        revision,
        path,
        side,
        start_line,
        line,
        body,
    } = request;
    let item = store
        .item(&id)?
        .ok_or_else(|| ReviewError::NoItem(id.clone()))?;
    reviewable(&item)?;
    if revision == 0 || revision > item.revision.number {
        return Err(ReviewError::NoRevision { id, revision });
    }
    let body = body.trim().to_owned();
    if body.is_empty() {
        return Err(ReviewError::EmptyComment);
    }
    let Some(file) = store
        .revision_files(&id, revision)?
        .into_iter()
        .find(|file| file.path == path)
    else {
        return Err(ReviewError::NoFile { id, revision, path });
    };
    // The diff calls a file binary as git does, and shows it no lines.
    let diff = store.diff(file.old_blob.as_deref(), file.new_blob.as_deref())?;
    if diff.is_some_and(|diff| diff.content == Content::Binary) {
        return Err(ReviewError::NotText(path));
    }
    let no_lines = || ReviewError::NoLines {
        path: path.clone(),
        side,
        start_line,
        line,
    };
    let blob = match side {
        Side::Old => file.old_blob,
        Side::New => file.new_blob,
    }
    .ok_or_else(no_lines)?;
    let bytes = git
        .blobs(id.project(), &[&blob])?
        .pop()
        .flatten()
        .ok_or_else(|| ReviewError::NotText(path.clone()))?;
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = text.lines().collect();
    if start_line == 0 || start_line > line || line as usize > lines.len() {
        return Err(no_lines());
    }
    let quote = quote(&lines, start_line, line);
    let comment = NewComment {
        item: id,
        revision,
        path,
        side,
        start_line,
        line,
        body,
        quote,
    };
    Ok(store.insert_draft(comment)?)
}

/// The last [`QUOTE_LINES`] of lines `start_line..=line`, which `lines` has.
fn quote(lines: &[&str], start_line: u32, line: u32) -> String {
    let from = (start_line as usize).max((line as usize).saturating_sub(QUOTE_LINES - 1));
    lines[from - 1..line as usize].join("\n")
}

/// Rewrites a draft's body; a sent comment stays as the agent read it.
pub fn edit_draft<S: ReviewStore>(
    store: &mut S,
    id: &ItemId,
    comment: CommentId,
    body: String,
) -> Result<Comment, ReviewError> {
    let mut draft = draft(store, id, comment)?;
    let body = body.trim().to_owned();
    if body.is_empty() {
        return Err(ReviewError::EmptyComment);
    }
    store.set_draft_body(comment, &body)?;
    draft.body = body;
    Ok(draft)
}

pub fn delete_draft<S: ReviewStore>(
    store: &mut S,
    id: &ItemId,
    comment: CommentId,
) -> Result<(), ReviewError> {
    draft(store, id, comment)?;
    Ok(store.delete_draft(comment)?)
}

fn draft<S: ReviewStore>(
    store: &S,
    id: &ItemId,
    comment: CommentId,
) -> Result<Comment, ReviewError> {
    let found = store
        .comments(id)?
        .into_iter()
        .find(|c| c.id == comment)
        .ok_or_else(|| ReviewError::NoDraft {
            id: id.clone(),
            comment,
        })?;
    match found.review {
        None => Ok(found),
        Some(_) => Err(ReviewError::Sent(comment)),
    }
}

#[cfg(test)]
#[path = "draft_tests.rs"]
mod tests;
