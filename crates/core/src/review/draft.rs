//! Draft comments: written on lines of the diff, kept on the hub until the
//! operator submits a review, which sends them all (design §9.2).

use super::{
    Comment, CommentId, NewComment, QUOTE_LINES, ReviewError, ReviewStore, Side, reviewable,
};
use crate::diff::{Content, DiffGit, DiffStore};
use crate::git::GitError;
use crate::item::{ItemId, ItemStore};
use crate::store::StoreError;

/// A quoted line longer than this is cut, so a minified file's line doesn't
/// fill the agent's context.
pub const QUOTE_LINE_CHARS: usize = 200;

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

#[derive(Debug, thiserror::Error)]
pub enum DraftError {
    /// The item can't take a review, so it can't take a draft either.
    #[error(transparent)]
    Review(#[from] ReviewError),
    #[error("a comment needs a body")]
    Empty,
    #[error("{id} r{revision} doesn't change {path}")]
    NoFile {
        id: ItemId,
        revision: u32,
        path: String,
    },
    #[error("{path} has no lines {start_line}-{line} on the {} side", side.as_str())]
    NoLines {
        path: String,
        side: Side,
        start_line: u32,
        line: u32,
    },
    #[error("{0} isn't text, so it has no lines to comment on")]
    NotText(String),
    #[error("{id} has no draft {comment}")]
    NoDraft { id: ItemId, comment: CommentId },
    #[error("{0} was sent, so it can't change")]
    Sent(CommentId),
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Drafts a comment on lines the revision's side of the file has. The comment
/// quotes them from the file, so it reads the same whatever the diff showed.
pub fn write_draft<S: ItemStore + DiffStore + ReviewStore>(
    store: &mut S,
    git: &impl DiffGit,
    request: WriteDraft,
) -> Result<Comment, DraftError> {
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
        return Err(ReviewError::NoRevision { id, revision }.into());
    }
    let body = body.trim().to_owned();
    if body.is_empty() {
        return Err(DraftError::Empty);
    }
    let Some(file) = store
        .revision_files(&id, revision)?
        .into_iter()
        .find(|file| file.path == path)
    else {
        return Err(DraftError::NoFile { id, revision, path });
    };
    // Submitting a revision caches every file's diff, which calls a file
    // binary as git does and shows it no lines.
    let diff = store
        .diff(file.old_blob.as_deref(), file.new_blob.as_deref())?
        .ok_or_else(|| StoreError::new(format!("{id} has no cached diff for {path}")))?;
    if diff.content == Content::Binary {
        return Err(DraftError::NotText(path));
    }
    let no_lines = || DraftError::NoLines {
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
        .ok_or_else(|| GitError::Failed(format!("{id} has no blob {blob} for {path}")))?;
    let quote = quote(&String::from_utf8_lossy(&bytes), start_line, line).ok_or_else(no_lines)?;
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

/// The last [`QUOTE_LINES`] of `text`'s lines `start_line..=line`, each cut
/// to [`QUOTE_LINE_CHARS`]; `None` when it has no such lines.
fn quote(text: &str, start_line: u32, line: u32) -> Option<String> {
    // Counted as git counts them: `\r\n` ends a line too, and a last line
    // without a newline is one.
    let lines: Vec<&str> = text.lines().collect();
    let (start_line, line) = (start_line as usize, line as usize);
    if start_line == 0 || start_line > line || line > lines.len() {
        return None;
    }
    let from = start_line.max(line.saturating_sub(QUOTE_LINES - 1));
    let cut =
        lines[from - 1..line]
            .iter()
            .map(|text| match text.char_indices().nth(QUOTE_LINE_CHARS) {
                Some((at, _)) => format!("{}…", &text[..at]),
                None => (*text).to_owned(),
            });
    Some(cut.collect::<Vec<_>>().join("\n"))
}

/// Rewrites a draft's body; a sent comment stays as the agent read it.
pub fn edit_draft<S: ItemStore + ReviewStore>(
    store: &mut S,
    id: &ItemId,
    comment: CommentId,
    body: String,
) -> Result<Comment, DraftError> {
    let mut draft = draft(store, id, comment)?;
    // It could never be sent; deleting it is all that's left.
    let item = store
        .item(id)?
        .ok_or_else(|| ReviewError::NoItem(id.clone()))?;
    reviewable(&item)?;
    let body = body.trim().to_owned();
    if body.is_empty() {
        return Err(DraftError::Empty);
    }
    store.set_draft_body(comment, &body)?;
    draft.body = body;
    Ok(draft)
}

pub fn delete_draft<S: ItemStore + ReviewStore>(
    store: &mut S,
    id: &ItemId,
    comment: CommentId,
) -> Result<(), DraftError> {
    draft(store, id, comment)?;
    Ok(store.delete_draft(comment)?)
}

fn draft<S: ItemStore + ReviewStore>(
    store: &S,
    id: &ItemId,
    comment: CommentId,
) -> Result<Comment, DraftError> {
    if store.item(id)?.is_none() {
        return Err(ReviewError::NoItem(id.clone()).into());
    }
    let found = store
        .comments(id)?
        .into_iter()
        .find(|c| c.id == comment)
        .ok_or_else(|| DraftError::NoDraft {
            id: id.clone(),
            comment,
        })?;
    match found.review {
        None => Ok(found),
        Some(_) => Err(DraftError::Sent(comment)),
    }
}

#[cfg(test)]
#[path = "draft_tests.rs"]
mod tests;
