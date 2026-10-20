//! Bringing a pull request's comments in: a new one is placed on a revision
//! and held for the next batch, an edited one held again, and a held one
//! GitHub deleted withdrawn. A bot's summary is kept for the operator to
//! read and never held, and neither is a bot's edit to a comment the agent
//! already answered.

use std::collections::{BTreeMap, BTreeSet};

use super::{ExternalError, ExternalStore, GithubKey, NewExternal, summary};
use crate::diff::{Content, DiffGit, DiffStore};
use crate::github::{Github, GithubError, GithubRepo, PullComment, PullLines};
use crate::item::{Item, ItemBackend, ItemId, ItemState, ItemStore, Revision};
use crate::review::{Anchor, Author, CommentId, ReviewError, ReviewStore, Side, quote, shown};

/// What an ingest changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Ingested {
    pub new: usize,
    pub edited: usize,
    /// Held ones GitHub no longer has.
    pub withdrawn: usize,
}

impl Ingested {
    pub fn changed(self) -> bool {
        self.new + self.edited + self.withdrawn > 0
    }
}

/// The repository and number of the item's pull request; none for an item
/// that isn't on GitHub.
pub fn pull_of(item: &Item) -> Option<(&GithubRepo, u64)> {
    match &item.backend {
        ItemBackend::Github { repo, pull } => Some((repo, pull.number)),
        ItemBackend::Local => None,
    }
}

/// The pull request's comments that are for the agent: not the operator's
/// own, who says what they want through reviews, and not empty, as a review
/// that only approves is.
pub fn fetch(
    github: &impl Github,
    repo: &GithubRepo,
    number: u64,
    fresh: bool,
) -> Result<Vec<PullComment>, GithubError> {
    let viewer = github.viewer()?;
    Ok(github
        .pull_comments(repo, number, fresh)?
        .into_iter()
        .filter(|c| kept(c, &viewer))
        .collect())
}

pub(super) fn kept(comment: &PullComment, viewer: &str) -> bool {
    !comment.login.eq_ignore_ascii_case(viewer) && !comment.body.trim().is_empty()
}

/// Records what `comments`, all of the pull request's that [`fetch`] keeps,
/// adds to `id`'s and holds it for the next batch. A held comment in `gone`
/// ([`confirm_gone`](super::confirm_gone)) and not in `comments` is withdrawn
/// ([`ExternalStore::withdraw_external`]); one a batch carried and nothing
/// holds stays as the agent got it, since the agent may have answered it.
pub fn ingest<S: ItemStore + DiffStore + ReviewStore + ExternalStore>(
    store: &mut S,
    git: &impl DiffGit,
    id: &ItemId,
    comments: Vec<PullComment>,
    gone: &BTreeSet<GithubKey>,
    now: u64,
) -> Result<Ingested, ExternalError> {
    let item = store
        .item(id)?
        .ok_or_else(|| ReviewError::NoItem(id.clone()))?;
    if matches!(item.state, ItemState::Merged | ItemState::Abandoned) {
        return Ok(Ingested::default());
    }
    let answered: BTreeSet<CommentId> = store
        .replies(id)?
        .into_iter()
        .filter(|r| r.author == Author::Agent)
        .map(|r| r.comment)
        .collect();
    let known: BTreeMap<GithubKey, Known> = store
        .comments(id)?
        .into_iter()
        .filter_map(|c| {
            let external = c.external?;
            let known = Known {
                answered: answered.contains(&c.id),
                id: c.id,
                body: c.body,
                pending: external.pending,
                summary: external.summary,
                bot: external.bot,
            };
            Some(((external.kind, external.github_id), known))
        })
        .collect();
    let revisions = store.revisions(id)?;
    let mut ingested = Ingested::default();
    let mut found = BTreeSet::new();
    for comment in comments {
        found.insert((comment.kind, comment.id));
        let known = known.get(&(comment.kind, comment.id));
        // One a batch carried stays the agent's, summary or not: its thread
        // and the agent's replies on it are already there.
        let reads_as_summary = summary(&comment);
        let summary = reads_as_summary && known.is_none_or(|k| k.pending || k.summary);
        match known {
            Some(known) if known.body == comment.body && known.summary == summary => {}
            // Bots edit what they posted to say where it stands, as "Addressed
            // in abc123" under a finding the agent answered, or turn a
            // placeholder into their summary. Held again, the edit would come
            // back to the agent as a comment to act on, and the operator would
            // vote on it again. A person's edit can ask for more, so it's held.
            Some(known)
                if known.bot
                    && !known.pending
                    && !known.summary
                    && (known.answered || reads_as_summary) =>
            {
                store.amend_external(known.id, &comment.body, comment.dated, now)?;
                ingested.edited += 1;
            }
            // Unedited, it's one the hub held before it told summaries apart:
            // counted as edited, as the operator should see it change.
            Some(known) => {
                store.edit_external(known.id, &comment.body, comment.dated, summary, now)?;
                ingested.edited += 1;
            }
            None => {
                let (revision, anchor) = match &comment.lines {
                    Some(lines) => place(store, git, &item, &revisions, lines)?,
                    None => (item.revision.number, None),
                };
                let new = NewExternal {
                    item: id.clone(),
                    revision,
                    anchor,
                    body: comment.body,
                    kind: comment.kind,
                    github_id: comment.id,
                    login: comment.login,
                    bot: comment.bot,
                    url: comment.url,
                    path: comment.lines.map(|lines| lines.path),
                    summary,
                    dated: comment.dated,
                };
                store.insert_external(new, now)?;
                ingested.new += 1;
            }
        }
    }
    for (key, known) in known {
        if known.pending && gone.contains(&key) && !found.contains(&key) {
            store.withdraw_external(known.id)?;
            ingested.withdrawn += 1;
        }
    }
    Ok(ingested)
}

/// An external comment as the hub has it.
struct Known {
    id: CommentId,
    body: String,
    pending: bool,
    summary: bool,
    bot: bool,
    /// The agent replied to it.
    answered: bool,
}

/// The revision an inline comment was written on and its lines there, where
/// the diff shows them as it does on GitHub; otherwise the item's newest
/// revision, with the comment on the item as a whole. Lines GitHub carried
/// to a later commit place it when the commit it was written on isn't a
/// revision's head, as when someone pushed to the branch.
fn place<S: ItemStore + DiffStore>(
    store: &S,
    git: &impl DiffGit,
    item: &Item,
    revisions: &[Revision],
    lines: &PullLines,
) -> Result<(u32, Option<Anchor>), ExternalError> {
    let side = if lines.old_side { Side::Old } else { Side::New };
    let tries = [
        (
            &lines.original_commit,
            lines.original_start_line,
            lines.original_line,
        ),
        (&lines.commit, lines.start_line, lines.line),
    ];
    for (commit, start_line, line) in tries {
        // A comment on the whole file, or lines GitHub couldn't carry, has
        // no line.
        let (Some(revision), Some(line)) = (revisions.iter().find(|r| r.head == *commit), line)
        else {
            continue;
        };
        let start_line = start_line.unwrap_or(line);
        let quoted = quote_lines(
            store,
            git,
            &item.id,
            revision.number,
            &lines.path,
            side,
            start_line,
            line,
        )?;
        if let Some(quote) = quoted {
            let anchor = Anchor {
                path: lines.path.clone(),
                side,
                start_line,
                line,
                quote,
            };
            return Ok((revision.number, Some(anchor)));
        }
    }
    Ok((item.revision.number, None))
}

/// The lines as a draft would quote them, when the revision's diff shows
/// them; the UI puts a comment under its line, so one on lines it doesn't
/// show would never be seen.
#[allow(clippy::too_many_arguments)]
fn quote_lines<S: ItemStore + DiffStore>(
    store: &S,
    git: &impl DiffGit,
    id: &ItemId,
    revision: u32,
    path: &str,
    side: Side,
    start_line: u32,
    line: u32,
) -> Result<Option<String>, ExternalError> {
    let Some(file) = store
        .revision_files(id, revision)?
        .into_iter()
        .map(|stat| stat.file)
        .find(|file| file.path == path)
    else {
        return Ok(None);
    };
    let diff = store.diff(file.old_blob.as_deref(), file.new_blob.as_deref())?;
    let Some(Content::Text(hunks)) = diff.map(|d| d.content) else {
        return Ok(None);
    };
    if !shown(&hunks, side, start_line, line) {
        return Ok(None);
    }
    let blob = match side {
        Side::Old => file.old_blob,
        Side::New => file.new_blob,
    };
    let Some(blob) = blob else {
        return Ok(None);
    };
    let Some(bytes) = git.blobs(id.project(), &[&blob])?.pop().flatten() else {
        return Ok(None);
    };
    Ok(quote(&String::from_utf8_lossy(&bytes), start_line, line))
}

#[cfg(test)]
#[path = "ingest_tests.rs"]
mod tests;
